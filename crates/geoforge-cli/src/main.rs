//! `gf`: GeoForge geoprocessing tools from the command line. Tool names and
//! behavior follow their ArcGIS Pro counterparts.
//!
//! Datasets are addressed the way ArcGIS does: `roads.shp`, `city.gdb`,
//! `city.gdb/roads` or `city.gdb/Transport/roads` (feature dataset optional).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use geoforge_core::{Error, Feature, FeatureClass, Field, FieldType, Result};
use geoforge_filegdb::Geodatabase;
use geoforge_shapefile::{ShapefileReader, ShapefileWriter};

#[derive(Parser)]
#[command(name = "gf", version, about = "GeoForge geoprocessing tools")]
struct Cli {
    #[command(subcommand)]
    tool: Tool,
}

#[derive(Subcommand)]
enum Tool {
    /// Print dataset properties, like arcpy.Describe.
    Describe { dataset: PathBuf },
    /// Copy features to a new feature class (ArcGIS "Copy Features").
    CopyFeatures {
        in_features: PathBuf,
        out_feature_class: PathBuf,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().tool {
        Tool::Describe { dataset } => describe(&dataset),
        Tool::CopyFeatures {
            in_features,
            out_feature_class,
        } => copy_features(&in_features, &out_feature_class),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}

/// A path split into its geodatabase and the item inside it, if any.
enum Location {
    Shapefile(PathBuf),
    Geodatabase { gdb: PathBuf, item: Option<String> },
}

fn locate(path: &Path) -> Location {
    let comps: Vec<_> = path.components().collect();
    let is_gdb_name = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb"));
    // Prefer the directory that really is a geodatabase; fall back to a `.gdb` name
    // (e.g. an output that does not exist yet).
    let mut prefix = PathBuf::new();
    let mut found = None;
    for (i, c) in comps.iter().enumerate() {
        prefix.push(c);
        if prefix.join("a00000001.gdbtable").is_file() {
            found = Some(i);
            break;
        }
        if found.is_none() && is_gdb_name(&prefix) && !prefix.is_dir() {
            found = Some(i);
        }
    }
    match found {
        Some(i) => {
            let gdb: PathBuf = comps[..=i].iter().collect();
            // The last component names the item; a feature dataset before it is optional.
            let item = comps[i + 1..]
                .last()
                .map(|c| c.as_os_str().to_string_lossy().into_owned());
            Location::Geodatabase { gdb, item }
        }
        None => Location::Shapefile(path.to_path_buf()),
    }
}

fn open_feature_class(path: &Path) -> Result<(Box<dyn FeatureClass>, &'static str)> {
    match locate(path) {
        Location::Shapefile(p) => Ok((Box::new(ShapefileReader::open(p)?), "ShapeFile")),
        Location::Geodatabase {
            gdb,
            item: Some(item),
        } => {
            let table = Geodatabase::open(&gdb)?.open_table(&item)?;
            let kind = if table.geometry_kind().is_some() {
                "FeatureClass"
            } else {
                "Table"
            };
            Ok((Box::new(table), kind))
        }
        Location::Geodatabase { gdb, item: None } => Err(Error::Unsupported(format!(
            "{} is a workspace; name a feature class inside it",
            gdb.display()
        ))),
    }
}

fn describe(path: &Path) -> Result<()> {
    if let Location::Geodatabase { gdb, item: None } = locate(path) {
        return describe_workspace(&gdb);
    }
    let (fc, data_type) = open_feature_class(path)?;
    let py = |b: bool| if b { "True" } else { "False" };
    println!("dataType:         {data_type}");
    if let Some(kind) = fc.geometry_kind() {
        println!("shapeType:        {kind:?}");
        println!("hasZ:             {}", py(fc.has_z()));
        println!("hasM:             {}", py(fc.has_m()));
        println!("shapeFieldName:   {}", fc.shape_field_name());
    }
    println!("OIDFieldName:     {}", fc.oid_field_name());
    println!("featureCount:     {}", fc.features().count());
    if let Some(e) = fc.stored_extent() {
        println!(
            "extent:           {} {} {} {}",
            e.xmin, e.ymin, e.xmax, e.ymax
        );
        if let Some((lo, hi)) = e.z {
            println!("extent Z:         {lo} {hi}");
        }
        if let Some((lo, hi)) = e.m {
            println!("extent M:         {lo} {hi}");
        }
    }
    if fc.geometry_kind().is_some() {
        let sr = fc
            .spatial_reference_wkt()
            .and_then(|wkt| wkt.split('"').nth(1))
            .unwrap_or("Unknown");
        println!("spatialReference: {sr}");
    }
    println!("fields:");
    println!("  {:<16} OID", fc.oid_field_name());
    if fc.geometry_kind().is_some() {
        println!("  {:<16} Geometry", fc.shape_field_name());
    }
    for f in fc.fields() {
        let detail = match f.field_type {
            FieldType::String => format!("length {}", f.length),
            FieldType::SmallInteger | FieldType::Integer | FieldType::Double if f.precision > 0 => {
                format!("precision {}, scale {}", f.precision, f.scale)
            }
            _ => String::new(),
        };
        let alias = f
            .alias
            .as_deref()
            .map(|a| format!(" alias \"{a}\""))
            .unwrap_or_default();
        let domain = f
            .domain
            .as_deref()
            .map(|d| format!(" domain \"{d}\""))
            .unwrap_or_default();
        println!(
            "  {:<16} {:<15} {detail}{alias}{domain}",
            f.name,
            esri_type_name(f.field_type)
        );
    }
    describe_subtypes(path)
}

fn describe_subtypes(path: &Path) -> Result<()> {
    let Location::Geodatabase {
        gdb,
        item: Some(item),
    } = locate(path)
    else {
        return Ok(());
    };
    let table = Geodatabase::open(&gdb)?.open_table(&item)?;
    if let Some(st) = table.subtypes() {
        println!(
            "subtypeFieldName: {} (default {})",
            st.field, st.default_code
        );
        for s in &st.subtypes {
            println!("  {:<6} {}", s.code, s.name);
            for f in &s.fields {
                let domain = f
                    .domain
                    .as_deref()
                    .map(|d| format!(" domain \"{d}\""))
                    .unwrap_or_default();
                let default = f
                    .default
                    .as_ref()
                    .map(|v| format!(" default {v:?}"))
                    .unwrap_or_default();
                println!("         {}{domain}{default}", f.field);
            }
        }
    }
    Ok(())
}

fn describe_workspace(gdb: &Path) -> Result<()> {
    let ws = Geodatabase::open(gdb)?;
    println!("dataType:         Workspace");
    println!("workspaceType:    LocalDatabase");
    println!("workspaceFactoryProgID: esriDataSourcesGDB.FileGDBWorkspaceFactory");
    if !ws.feature_datasets().is_empty() {
        println!("feature datasets: {}", ws.feature_datasets().join(", "));
    }
    if !ws.domains().is_empty() {
        println!("domains:");
        for d in ws.domains() {
            let kind = match &d.kind {
                geoforge_filegdb::DomainKind::Coded(v) => format!("coded, {} values", v.len()),
                geoforge_filegdb::DomainKind::Range { min, max } => {
                    format!("range {min:?} .. {max:?}")
                }
            };
            println!(
                "  {:<40} {:<13} {kind}",
                d.name,
                esri_type_name(d.field_type)
            );
        }
    }
    if !ws.relationship_classes().is_empty() {
        println!("relationship classes:");
        for r in ws.relationship_classes() {
            println!(
                "  {:<32} {:?}{} {} -> {} ({} = {})",
                r.name,
                r.cardinality,
                if r.is_composite { " composite" } else { "" },
                r.origin,
                r.destination,
                r.origin_primary_key,
                r.origin_foreign_key
            );
        }
    }
    println!("children:");
    for e in ws.entries() {
        let path = match &e.feature_dataset {
            Some(fd) => format!("{fd}/{}", e.name),
            None => e.name.clone(),
        };
        let kind = match ws.open_table(&e.name) {
            Ok(t) => match t.geometry_kind() {
                Some(k) => format!(
                    "FeatureClass ({k:?}{}{})",
                    if t.has_z() { " Z" } else { "" },
                    if t.has_m() { " M" } else { "" }
                ),
                None => "Table".into(),
            },
            Err(err) => format!("unreadable: {err}"),
        };
        println!("  {path:<40} {kind}");
    }
    Ok(())
}

/// The field type names ArcGIS Pro shows (`Field.type`).
fn esri_type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "SmallInteger",
        FieldType::Integer => "Integer",
        FieldType::BigInteger => "BigInteger",
        FieldType::Single => "Single",
        FieldType::Double => "Double",
        FieldType::String => "String",
        FieldType::Date => "Date",
        FieldType::DateOnly => "DateOnly",
        FieldType::TimeOnly => "TimeOnly",
        FieldType::TimestampOffset => "TimestampOffset",
        FieldType::ObjectId => "OID",
        FieldType::Geometry => "Geometry",
        FieldType::Blob => "Blob",
        FieldType::Raster => "Raster",
        FieldType::Guid => "Guid",
        FieldType::GlobalId => "GlobalID",
        FieldType::Xml => "XML",
    }
}

/// What ArcGIS Copy Features does with a field when writing a shapefile
/// (observed in ArcGIS Pro 3.7): store it, drop it with WARNING 000592, or fail the
/// tool with ERROR 000308.
enum ShapefileField {
    Store,
    Drop,
    Fail,
}

fn shapefile_field(t: FieldType) -> ShapefileField {
    match t {
        FieldType::Blob | FieldType::Raster => ShapefileField::Drop,
        FieldType::BigInteger
        | FieldType::Xml
        | FieldType::TimeOnly
        | FieldType::TimestampOffset => ShapefileField::Fail,
        _ => ShapefileField::Store,
    }
}

/// ArcGIS `ValidateFieldName` for file geodatabases: invalid characters become
/// `_`, names must start with a letter, at most 64 characters, unique.
fn geodatabase_field_names(fields: &[Field]) -> Vec<String> {
    let mut seen: Vec<String> = vec!["OBJECTID".into(), "SHAPE".into()];
    fields
        .iter()
        .map(|f| {
            let mut n: String = f
                .name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if !n.chars().next().is_some_and(char::is_alphabetic) {
                n.insert(0, 'F');
            }
            n = n.chars().take(64).collect();
            let base = n.clone();
            let mut i = 1;
            while seen.iter().any(|s| s.eq_ignore_ascii_case(&n)) {
                n = format!("{base}_{i}");
                i += 1;
            }
            seen.push(n.clone());
            n
        })
        .collect()
}

fn copy_to_geodatabase(
    fc: &dyn FeatureClass,
    source_gdb: Option<&Path>,
    kind: geoforge_core::GeometryKind,
    gdb: &Path,
    name: &str,
) -> Result<()> {
    if gdb.exists() {
        return Err(Error::Unsupported(format!(
            "{} exists; adding to an existing geodatabase is not implemented yet",
            gdb.display()
        )));
    }
    let names = geodatabase_field_names(fc.fields());
    let fields: Vec<Field> = fc
        .fields()
        .iter()
        .zip(&names)
        .map(|(f, n)| Field {
            name: n.clone(),
            ..f.clone()
        })
        .collect();
    let mut w = geoforge_filegdb::GeodatabaseWriter::create(gdb)?;
    // Like ArcGIS, carry over the attribute domains the copied fields use.
    if let Some(source) = source_gdb {
        let src = Geodatabase::open(source)?;
        let mut added: Vec<&str> = Vec::new();
        for name in fields.iter().filter_map(|f| f.domain.as_deref()) {
            if let Some(d) = src.domain(name).filter(|_| !added.contains(&name)) {
                w.add_domain(d)?;
                added.push(name);
            }
        }
    }
    let mut t = w.create_table(
        name,
        Some(kind),
        fc.has_z(),
        fc.has_m(),
        &fields,
        fc.spatial_reference_wkt(),
    )?;
    let mut n = 0;
    for feature in fc.features() {
        t.write(&feature?)?;
        n += 1;
    }
    w.finish_table(t)?;
    w.finish()?;
    println!("Copied {n} features to {}", gdb.join(name).display());
    Ok(())
}

fn copy_features(input: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let Some(kind) = fc.geometry_kind() else {
        return Err(Error::Unsupported(
            "input has no geometry; use Copy Rows for tables".into(),
        ));
    };
    if let Location::Geodatabase { gdb, item } = locate(output) {
        let item = item.ok_or_else(|| {
            Error::Unsupported("name the output feature class inside the .gdb".into())
        })?;
        let source_gdb = match locate(input) {
            Location::Geodatabase { gdb, .. } => Some(gdb),
            Location::Shapefile(_) => None,
        };
        return copy_to_geodatabase(fc.as_ref(), source_gdb.as_deref(), kind, &gdb, &item);
    }

    let mut keep = Vec::new();
    for (i, f) in fc.fields().iter().enumerate() {
        match shapefile_field(f.field_type) {
            ShapefileField::Store => keep.push(i),
            ShapefileField::Drop => eprintln!(
                "WARNING 000592: Output format does not support BLOB, Raster, or GUID fields. Field(s) {} will not be converted to output.",
                f.name
            ),
            ShapefileField::Fail => {
                return Err(Error::Unsupported(format!(
                    "ERROR 000308: Invalid field type: field {} ({}) cannot be stored in a shapefile",
                    f.name,
                    esri_type_name(f.field_type)
                )));
            }
        }
    }
    let fields: Vec<Field> = keep.iter().map(|&i| fc.fields()[i].clone()).collect();
    let mut w = ShapefileWriter::create(
        output,
        kind,
        fc.has_z(),
        fc.has_m(),
        &fields,
        fc.spatial_reference_wkt(),
    )?;
    let (mut n, mut skipped) = (0, Vec::new());
    for feature in fc.features() {
        let f = feature?;
        let attributes = keep.iter().map(|&i| f.attributes[i].clone()).collect();
        match w.write(&Feature {
            oid: f.oid,
            geometry: f.geometry,
            attributes,
        }) {
            Ok(()) => n += 1,
            // ArcGIS skips rows whose values cannot be stored and carries on.
            Err(Error::BadValue(why)) => {
                eprintln!(
                    "WARNING 160277: The row contains a bad value. ({} {}: {why})",
                    fc.oid_field_name(),
                    f.oid
                );
                skipped.push(f.oid);
            }
            Err(e) => return Err(e),
        }
    }
    if !skipped.is_empty() {
        eprintln!(
            "WARNING 000595: {} records could not be processed: {skipped:?}",
            skipped.len()
        );
    }
    w.finish()?;
    println!("Copied {n} features to {}", output.display());
    Ok(())
}
