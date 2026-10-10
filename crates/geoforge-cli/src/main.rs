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
        println!(
            "  {:<16} {:<15} {detail}{alias}",
            f.name,
            esri_type_name(f.field_type)
        );
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

/// Field types a shapefile cannot hold; ArcGIS drops these on export with a warning.
fn shapefile_can_store(t: FieldType) -> bool {
    !matches!(
        t,
        FieldType::Blob | FieldType::Raster | FieldType::TimeOnly | FieldType::TimestampOffset
    )
}

fn copy_features(input: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let Some(kind) = fc.geometry_kind() else {
        return Err(Error::Unsupported(
            "input has no geometry; use Copy Rows for tables".into(),
        ));
    };
    if !matches!(locate(output), Location::Shapefile(_)) {
        return Err(Error::Unsupported(
            "writing file geodatabases is not implemented yet".into(),
        ));
    }

    let keep: Vec<usize> = (0..fc.fields().len())
        .filter(|&i| {
            let f = &fc.fields()[i];
            let ok = shapefile_can_store(f.field_type);
            if !ok {
                eprintln!(
                    "WARNING: field {} ({}) is not supported by shapefiles and was dropped",
                    f.name,
                    esri_type_name(f.field_type)
                );
            }
            ok
        })
        .collect();
    let fields: Vec<Field> = keep.iter().map(|&i| fc.fields()[i].clone()).collect();
    let mut w = ShapefileWriter::create(
        output,
        kind,
        fc.has_z(),
        fc.has_m(),
        &fields,
        fc.spatial_reference_wkt(),
    )?;
    let mut n = 0;
    for feature in fc.features() {
        let f = feature?;
        let attributes = keep.iter().map(|&i| f.attributes[i].clone()).collect();
        w.write(&Feature {
            oid: f.oid,
            geometry: f.geometry,
            attributes,
        })?;
        n += 1;
    }
    w.finish()?;
    println!("Copied {n} features to {}", output.display());
    Ok(())
}
