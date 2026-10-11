//! `gf`: GeoForge geoprocessing tools from the command line. Tool names and
//! behavior follow their ArcGIS Pro counterparts.
//!
//! Datasets are addressed the way ArcGIS does: `roads.shp`, `city.gdb`,
//! `city.gdb/roads` or `city.gdb/Transport/roads` (feature dataset optional).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use geoforge_core::{Error, Feature, FeatureClass, Field, FieldType, Result, Value};
use geoforge_filegdb::Geodatabase;
use geoforge_shapefile::{ShapefileReader, ShapefileWriter};
use geoforge_tools::{Dissolve, ToolOutput};

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
    /// Extract the parts of input features inside clip polygons (ArcGIS "Clip").
    Clip {
        in_features: PathBuf,
        clip_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Geometric intersection of inputs (ArcGIS "Intersect"): `gf intersect a b ... out`.
    Intersect {
        /// Two or more inputs followed by the output.
        #[arg(num_args = 3.., required = true)]
        paths: Vec<PathBuf>,
        /// ALL, NO_FID or ONLY_FID.
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// INPUT (lowest input dimension), LINE or POINT.
        #[arg(long, default_value = "INPUT")]
        output_type: String,
    },
    /// Planar overlay of polygon inputs (ArcGIS "Union"): `gf union a b ... out`.
    Union {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// ALL, NO_FID or ONLY_FID.
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// GAPS, or NO_GAPS to add the enclosed areas no input covers.
        #[arg(long, default_value = "GAPS")]
        gaps: String,
    },
    /// Remove the parts of input features inside erase polygons (ArcGIS "Erase").
    Erase {
        in_features: PathBuf,
        erase_features: PathBuf,
        out_feature_class: PathBuf,
    },
    /// Split input features by identity polygons, joining their attributes
    /// (ArcGIS "Identity").
    Identity {
        in_features: PathBuf,
        identity_features: PathBuf,
        out_feature_class: PathBuf,
        /// ALL, NO_FID or ONLY_FID (for the identity features' columns).
        #[arg(long, default_value = "ALL")]
        join_attributes: String,
        /// NO_RELATIONSHIPS, or KEEP_RELATIONSHIPS for LEFT_/RIGHT_ columns (lines).
        #[arg(long, default_value = "NO_RELATIONSHIPS")]
        relationship: String,
    },
    /// Combine inputs of the same geometry type into one (ArcGIS "Merge"):
    /// `gf merge a b ... out`.
    Merge {
        /// One or more inputs followed by the output.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// NO_SOURCE_INFO (default) or ADD_SOURCE_INFO (adds MERGE_SRC).
        #[arg(long, default_value = "NO_SOURCE_INFO")]
        add_source: String,
    },
    /// Add the distance to the closest near feature (ArcGIS "Near", planar).
    /// Updates the input shapefile in place, as ArcGIS does, unless an output is
    /// given.
    Near {
        in_features: PathBuf,
        /// One near feature class, or several joined with ';' (adds NEAR_FC).
        near_features: String,
        /// Write the result here instead of updating the input.
        out_feature_class: Option<PathBuf>,
        /// Only consider near features within this distance, e.g. "100 Meters".
        #[arg(long)]
        search_radius: Option<String>,
        /// LOCATION: add NEAR_X and NEAR_Y.
        #[arg(long, default_value = "NO_LOCATION")]
        location: String,
        /// ANGLE: add NEAR_ANGLE.
        #[arg(long, default_value = "NO_ANGLE")]
        angle: String,
    },
    /// Join attributes by spatial relationship (ArcGIS "Spatial Join").
    SpatialJoin {
        target_features: PathBuf,
        join_features: PathBuf,
        out_feature_class: PathBuf,
        /// JOIN_ONE_TO_ONE or JOIN_ONE_TO_MANY.
        #[arg(long, default_value = "JOIN_ONE_TO_ONE")]
        join_operation: String,
        /// KEEP_ALL or KEEP_COMMON.
        #[arg(long, default_value = "KEEP_ALL")]
        join_type: String,
        /// INTERSECT, WITHIN_A_DISTANCE, CONTAINS, COMPLETELY_CONTAINS, WITHIN,
        /// COMPLETELY_WITHIN or CLOSEST.
        #[arg(long, default_value = "INTERSECT")]
        match_option: String,
        #[arg(long)]
        search_radius: Option<String>,
        /// Add a field with the distance to the joined feature.
        #[arg(long)]
        distance_field_name: Option<String>,
    },
    /// Write the input features that relate to selecting features (ArcGIS "Select
    /// Layer By Location" with a new selection, then "Copy Features").
    SelectByLocation {
        in_features: PathBuf,
        /// INTERSECT, WITHIN_A_DISTANCE, CONTAINS, COMPLETELY_CONTAINS,
        /// CONTAINS_CLEMENTINI, WITHIN, COMPLETELY_WITHIN, WITHIN_CLEMENTINI,
        /// ARE_IDENTICAL_TO, BOUNDARY_TOUCHES, SHARE_A_LINE_SEGMENT_WITH,
        /// CROSSED_BY_THE_OUTLINE_OF or HAVE_THEIR_CENTER_IN.
        overlap_type: String,
        select_features: PathBuf,
        out_feature_class: PathBuf,
        /// For INTERSECT and WITHIN_A_DISTANCE, e.g. "30 Meters".
        #[arg(long)]
        search_distance: Option<String>,
        /// INVERT: select the features that do not relate.
        #[arg(long, default_value = "NOT_INVERT")]
        invert_spatial_relationship: String,
    },
    /// Add the features of inputs to an existing target (ArcGIS "Append"):
    /// `gf append a b ... target`. Updates the target shapefile in place.
    Append {
        /// One or more inputs followed by the target.
        #[arg(num_args = 2.., required = true)]
        paths: Vec<PathBuf>,
        /// TEST (schemas must match) or NO_TEST (fields matched by name).
        #[arg(long, default_value = "TEST")]
        schema_type: String,
        /// Write the result here instead of updating the target.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Clip the input by each group of split polygons sharing a text value, one
    /// output per value named after it (ArcGIS "Split").
    Split {
        in_features: PathBuf,
        split_features: PathBuf,
        split_field: String,
        /// A folder; outputs are `<value>.shp`.
        out_workspace: PathBuf,
    },
    /// Write the `.sbn`/`.sbx` spatial index of a shapefile (ArcGIS "Add Spatial
    /// Index"); gf writes one with every shapefile anyway.
    AddSpatialIndex { in_features: PathBuf },
    /// List geometry problems (ArcGIS "Check Geometry") into a .dbf table:
    /// CLASS, FEATURE_ID, PROBLEM.
    CheckGeometry {
        /// One feature class, or several joined with ';'.
        in_features: String,
        out_table: PathBuf,
        /// ESRI or OGC.
        #[arg(long, default_value = "ESRI")]
        validation_method: String,
    },
    /// Repair geometry problems (ArcGIS "Repair Geometry"). Updates the input
    /// shapefile in place, as ArcGIS does, unless an output is given.
    RepairGeometry {
        in_features: PathBuf,
        /// DELETE_NULL or KEEP_NULL.
        #[arg(long, default_value = "DELETE_NULL")]
        delete_null: String,
        /// ESRI or OGC.
        #[arg(long, default_value = "ESRI")]
        validation_method: String,
        /// Write the result here instead of updating the input.
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Merge features by attribute values (ArcGIS "Dissolve").
    Dissolve {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// Field(s) to group by (repeatable). None: dissolve everything into one.
        #[arg(long = "dissolve-field")]
        dissolve_fields: Vec<String>,
        /// Statistics as ArcGIS writes them: "VALUE SUM;PARCEL_ID COUNT".
        #[arg(long)]
        statistics_fields: Option<String>,
        /// MULTI_PART (default) or SINGLE_PART.
        #[arg(long, default_value = "MULTI_PART")]
        multi_part: String,
    },
    /// Buffer features by a distance (ArcGIS "Buffer", planar).
    Buffer {
        in_features: PathBuf,
        out_feature_class: PathBuf,
        /// Linear distance, e.g. "25 Meters" (negative shrinks polygons).
        #[arg(allow_hyphen_values = true)]
        buffer_distance: String,
        /// ROUND or FLAT ends for lines.
        #[arg(long, default_value = "ROUND")]
        line_end_type: String,
        /// NONE (one buffer per feature) or ALL (dissolve into one).
        #[arg(long, default_value = "NONE")]
        dissolve_option: String,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().tool {
        Tool::Describe { dataset } => describe(&dataset),
        Tool::CopyFeatures {
            in_features,
            out_feature_class,
        } => copy_features(&in_features, &out_feature_class),
        Tool::Clip {
            in_features,
            clip_features,
            out_feature_class,
        } => clip_tool(&in_features, &clip_features, &out_feature_class),
        Tool::Intersect {
            mut paths,
            join_attributes,
            output_type,
        } => (|| {
            let out = paths.pop().unwrap();
            let opts = geoforge_tools::IntersectOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                output_type: geoforge_tools::OutputType::parse(&output_type)
                    .ok_or_else(|| Error::Unsupported(format!("output_type {output_type}")))?,
            };
            overlay_tool(&paths, &out, |i| geoforge_tools::intersect_with(i, &opts))
        })(),
        Tool::Union {
            mut paths,
            join_attributes,
            gaps,
        } => (|| {
            let out = paths.pop().unwrap();
            let join = join_attributes_arg(&join_attributes)?;
            let gaps = match gaps.to_ascii_uppercase().as_str() {
                "GAPS" => true,
                "NO_GAPS" => false,
                other => return Err(Error::Unsupported(format!("gaps {other}"))),
            };
            overlay_tool(&paths, &out, |i| geoforge_tools::union_with(i, join, gaps))
        })(),
        Tool::Erase {
            in_features,
            erase_features,
            out_feature_class,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (er, _) = open_feature_class(&erase_features)?;
            write_output(
                geoforge_tools::erase(fc.as_ref(), er.as_ref())?,
                &out_feature_class,
                source_gdb(&in_features).as_deref(),
            )
        })(),
        Tool::Identity {
            in_features,
            identity_features,
            out_feature_class,
            join_attributes,
            relationship,
        } => (|| {
            let opts = geoforge_tools::IdentityOptions {
                join_attributes: join_attributes_arg(&join_attributes)?,
                keep_relationships: match relationship.to_ascii_uppercase().as_str() {
                    "NO_RELATIONSHIPS" => false,
                    "KEEP_RELATIONSHIPS" => true,
                    other => return Err(Error::Unsupported(format!("relationship {other}"))),
                },
            };
            let (fc, _) = open_feature_class(&in_features)?;
            let (id, _) = open_feature_class(&identity_features)?;
            let out = geoforge_tools::identity_with(
                geoforge_tools::NamedInput {
                    name: dataset_name(&in_features),
                    features: fc.as_ref(),
                },
                geoforge_tools::NamedInput {
                    name: dataset_name(&identity_features),
                    features: id.as_ref(),
                },
                &opts,
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Merge {
            mut paths,
            add_source,
        } => {
            let out = paths.pop().unwrap();
            merge_tool(&paths, &out, &add_source)
        }
        Tool::Near {
            in_features,
            near_features,
            out_feature_class,
            search_radius,
            location,
            angle,
        } => near_tool(
            &in_features,
            &near_features,
            out_feature_class.as_deref(),
            search_radius.as_deref(),
            &location,
            &angle,
        ),
        Tool::SpatialJoin {
            target_features,
            join_features,
            out_feature_class,
            join_operation,
            join_type,
            match_option,
            search_radius,
            distance_field_name,
        } => (|| {
            let (target, _) = open_feature_class(&target_features)?;
            let (join, _) = open_feature_class(&join_features)?;
            let opts = geoforge_tools::SpatialJoinOptions {
                one_to_many: match join_operation.to_ascii_uppercase().as_str() {
                    "JOIN_ONE_TO_ONE" => false,
                    "JOIN_ONE_TO_MANY" => true,
                    other => return Err(Error::Unsupported(format!("join_operation {other}"))),
                },
                keep_all: match join_type.to_ascii_uppercase().as_str() {
                    "KEEP_ALL" => true,
                    "KEEP_COMMON" => false,
                    other => return Err(Error::Unsupported(format!("join_type {other}"))),
                },
                match_option: geoforge_tools::MatchOption::parse(&match_option)
                    .ok_or_else(|| Error::Unsupported(format!("match_option {match_option}")))?,
                search_radius: search_radius
                    .as_deref()
                    .map(|r| parse_distance(r, target.spatial_reference_wkt()))
                    .transpose()?,
                distance_field_name,
            };
            let out = geoforge_tools::spatial_join(target.as_ref(), join.as_ref(), &opts)?;
            write_output(
                out,
                &out_feature_class,
                source_gdb(&target_features).as_deref(),
            )
        })(),
        Tool::SelectByLocation {
            in_features,
            overlap_type,
            select_features,
            out_feature_class,
            search_distance,
            invert_spatial_relationship,
        } => (|| {
            let (fc, _) = open_feature_class(&in_features)?;
            let (sel, _) = open_feature_class(&select_features)?;
            let overlap = geoforge_tools::Overlap::parse(&overlap_type)
                .ok_or_else(|| Error::Unsupported(format!("overlap_type {overlap_type}")))?;
            let invert = match invert_spatial_relationship.to_ascii_uppercase().as_str() {
                "INVERT" => true,
                "NOT_INVERT" => false,
                other => return Err(Error::Unsupported(format!("invert {other}"))),
            };
            let distance = search_distance
                .as_deref()
                .map(|d| parse_distance(d, fc.spatial_reference_wkt()))
                .transpose()?;
            let out = geoforge_tools::select_by_location(
                fc.as_ref(),
                overlap,
                sel.as_ref(),
                distance,
                invert,
            )?;
            write_output(out, &out_feature_class, source_gdb(&in_features).as_deref())
        })(),
        Tool::Append {
            mut paths,
            schema_type,
            output,
        } => (|| {
            let target_path = paths.pop().unwrap();
            let test = match schema_type.to_ascii_uppercase().as_str() {
                "TEST" => true,
                "NO_TEST" => false,
                other => return Err(Error::Unsupported(format!("schema_type {other}"))),
            };
            let (target, _) = open_feature_class(&target_path)?;
            let inputs: Vec<Box<dyn FeatureClass>> = paths
                .iter()
                .map(|p| open_feature_class(p).map(|(f, _)| f))
                .collect::<Result<_>>()?;
            let refs: Vec<&dyn FeatureClass> = inputs.iter().map(|b| b.as_ref()).collect();
            let out = geoforge_tools::append(&refs, target.as_ref(), test)?;
            drop(target);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&target_path).as_deref()),
                None => update_in_place(out, &target_path),
            }
        })(),
        Tool::Split {
            in_features,
            split_features,
            split_field,
            out_workspace,
        } => split_tool(&in_features, &split_features, &split_field, &out_workspace),
        Tool::AddSpatialIndex { in_features } => (|| {
            let Location::Shapefile(shp) = locate(&in_features) else {
                return Err(Error::Unsupported(
                    "geodatabase feature classes always have a spatial index".into(),
                ));
            };
            let bytes = std::fs::read(&shp).map_err(|e| Error::io(&shp, e))?;
            let (records, extent, entries) = geoforge_shapefile::sbn::entries_from_shp(&bytes)?;
            geoforge_shapefile::sbn::write(&shp, records, extent, &entries)?;
            println!("Indexed {} ({} shapes)", shp.display(), entries.len());
            Ok(())
        })(),
        Tool::CheckGeometry {
            in_features,
            out_table,
            validation_method,
        } => check_geometry_tool(&in_features, &out_table, &validation_method),
        Tool::RepairGeometry {
            in_features,
            delete_null,
            validation_method,
            output,
        } => (|| {
            let delete_null = match delete_null.to_ascii_uppercase().as_str() {
                "DELETE_NULL" => true,
                "KEEP_NULL" => false,
                other => return Err(Error::Unsupported(format!("delete_null {other}"))),
            };
            let method = validation_method_arg(&validation_method)?;
            let (fc, _) = open_feature_class(&in_features)?;
            let out = geoforge_tools::repair_geometry(fc.as_ref(), delete_null, method)?;
            drop(fc);
            match output {
                Some(o) => write_output(out, &o, source_gdb(&in_features).as_deref()),
                None => update_in_place(out, &in_features),
            }
        })(),
        Tool::Dissolve {
            in_features,
            out_feature_class,
            dissolve_fields,
            statistics_fields,
            multi_part,
        } => dissolve_tool(
            &in_features,
            &out_feature_class,
            &dissolve_fields,
            statistics_fields.as_deref(),
            &multi_part,
        ),
        Tool::Buffer {
            in_features,
            out_feature_class,
            buffer_distance,
            line_end_type,
            dissolve_option,
        } => buffer_tool(
            &in_features,
            &out_feature_class,
            &buffer_distance,
            &line_end_type,
            &dissolve_option,
        ),
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

/// Writes a tool's output to a shapefile or `path.gdb/name`, applying the target
/// format's ArcGIS rules.
fn write_output(out: ToolOutput, output: &Path, source_gdb: Option<&Path>) -> Result<()> {
    for m in &out.messages {
        eprintln!("{m}");
    }
    match locate(output) {
        Location::Geodatabase { gdb, item } => {
            let item = item.ok_or_else(|| {
                Error::Unsupported("name the output feature class inside the .gdb".into())
            })?;
            write_geodatabase(out, &gdb, &item, source_gdb)
        }
        Location::Shapefile(path) => {
            let n = write_shapefile(out, &path)?;
            println!("Wrote {n} features to {}", path.display());
            Ok(())
        }
    }
}

fn write_geodatabase(
    out: ToolOutput,
    gdb: &Path,
    name: &str,
    source_gdb: Option<&Path>,
) -> Result<()> {
    if gdb.exists() {
        return Err(Error::Unsupported(format!(
            "{} exists; adding to an existing geodatabase is not implemented yet",
            gdb.display()
        )));
    }
    let names = geodatabase_field_names(&out.fields);
    let fields: Vec<Field> = out
        .fields
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
        Some(out.kind),
        out.has_z,
        out.has_m,
        &fields,
        out.spatial_reference_wkt.as_deref(),
    )?;
    let n = out.features.len();
    for f in &out.features {
        t.write(f)?;
    }
    w.finish_table(t)?;
    w.finish()?;
    println!("Wrote {n} features to {}", gdb.join(name).display());
    Ok(())
}

/// Writes a shapefile; returns the number of features written.
fn write_shapefile(out: ToolOutput, path: &Path) -> Result<usize> {
    let mut keep = Vec::new();
    for (i, f) in out.fields.iter().enumerate() {
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
    let mut fields: Vec<Field> = keep.iter().map(|&i| out.fields[i].clone()).collect();
    // A .dbf needs a field; ArcGIS's default for a new shapefile is "Id" N(6).
    let default_id = fields.is_empty();
    if default_id {
        let mut id = Field::new("Id", FieldType::Integer);
        id.precision = 6;
        fields.push(id);
    }
    let mut w = ShapefileWriter::create(
        path,
        out.kind,
        out.has_z,
        out.has_m,
        &fields,
        out.spatial_reference_wkt.as_deref(),
    )?;
    let (mut n, mut skipped) = (0, Vec::new());
    for f in out.features {
        let attributes = if default_id {
            vec![Value::Integer(0)]
        } else {
            keep.iter().map(|&i| f.attributes[i].clone()).collect()
        };
        match w.write(&Feature {
            oid: f.oid,
            geometry: f.geometry,
            attributes,
        }) {
            Ok(()) => n += 1,
            // ArcGIS skips rows whose values cannot be stored and carries on.
            Err(Error::BadValue(why)) => {
                eprintln!(
                    "WARNING 160277: The row contains a bad value. (OID {}: {why})",
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
    Ok(n)
}

fn source_gdb(input: &Path) -> Option<PathBuf> {
    match locate(input) {
        Location::Geodatabase { gdb, .. } => Some(gdb),
        Location::Shapefile(_) => None,
    }
}

/// Copy Features: the input unchanged, written to the output format.
fn copy_features(input: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let kind = fc.geometry_kind().ok_or_else(|| {
        Error::Unsupported("input has no geometry; use Copy Rows for tables".into())
    })?;
    let out = ToolOutput {
        kind,
        has_z: fc.has_z(),
        has_m: fc.has_m(),
        fields: fc.fields().to_vec(),
        spatial_reference_wkt: fc.spatial_reference_wkt().map(str::to_owned),
        features: fc.features().collect::<Result<_>>()?,
        messages: Vec::new(),
    };
    write_output(out, output, source_gdb(input).as_deref())
}

fn clip_tool(input: &Path, clip_features: &Path, output: &Path) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let (clip, _) = open_feature_class(clip_features)?;
    let out = geoforge_tools::clip(fc.as_ref(), clip.as_ref())?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// The name ArcGIS uses for a dataset in output field names (`FID_<name>`).
fn dataset_name(path: &Path) -> String {
    match locate(path) {
        Location::Geodatabase {
            item: Some(item), ..
        } => item,
        _ => path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}

fn overlay_tool(
    inputs: &[PathBuf],
    output: &Path,
    tool: impl Fn(&[geoforge_tools::NamedInput]) -> Result<ToolOutput>,
) -> Result<()> {
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let named: Vec<geoforge_tools::NamedInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::NamedInput {
            name: dataset_name(p),
            features: f.as_ref(),
        })
        .collect();
    write_output(tool(&named)?, output, source_gdb(&inputs[0]).as_deref())
}

fn near_tool(
    input: &Path,
    near_features: &str,
    output: Option<&Path>,
    search_radius: Option<&str>,
    location: &str,
    angle: &str,
) -> Result<()> {
    let flag = |value: &str, yes: &str, no: &str| match value.to_ascii_uppercase() {
        v if v == yes => Ok(true),
        v if v == no => Ok(false),
        v => Err(Error::Unsupported(format!(
            "expected {yes} or {no}, got {v}"
        ))),
    };
    let location = flag(location, "LOCATION", "NO_LOCATION")?;
    let angle = flag(angle, "ANGLE", "NO_ANGLE")?;
    let (fc, _) = open_feature_class(input)?;
    let near_paths: Vec<PathBuf> = near_features
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    let opened: Vec<Box<dyn FeatureClass>> = near_paths
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let near_inputs: Vec<geoforge_tools::PathInput> = near_paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let radius = search_radius
        .map(|r| parse_distance(r, fc.spatial_reference_wkt()))
        .transpose()?;
    let out = geoforge_tools::near(fc.as_ref(), &near_inputs, radius, location, angle)?;
    drop(fc);
    if let Some(output) = output {
        return write_output(out, output, source_gdb(input).as_deref());
    }
    update_in_place(out, input)
}

/// Replaces a shapefile with a tool's output (tools that edit their input, like
/// Near and Append): written next to it, then moved over its files.
fn update_in_place(out: ToolOutput, dataset: &Path) -> Result<()> {
    let Location::Shapefile(shp) = locate(dataset) else {
        return Err(Error::Unsupported(
            "updating a geodatabase feature class in place is not implemented yet; give an output"
                .into(),
        ));
    };
    let stem = shp
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = shp.with_file_name(format!("{stem}.gf-tmp.shp"));
    for m in &out.messages {
        eprintln!("{m}");
    }
    let n = write_shapefile(out, &tmp)?;
    // The new spatial index replaces the old one with the rest.
    for ext in ["shp", "shx", "dbf", "prj", "cpg", "sbn", "sbx"] {
        let from = tmp.with_extension(ext);
        if from.exists() {
            std::fs::rename(&from, shp.with_extension(ext)).map_err(|e| Error::io(&from, e))?;
        }
    }
    println!("Updated {} ({n} features)", shp.display());
    Ok(())
}

/// A dataset's full path, as ArcGIS records it in fields like `MERGE_SRC`.
fn catalog_path(p: &Path) -> String {
    std::path::absolute(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .display()
        .to_string()
}

fn join_attributes_arg(s: &str) -> Result<geoforge_tools::JoinAttributes> {
    geoforge_tools::JoinAttributes::parse(s)
        .ok_or_else(|| Error::Unsupported(format!("join_attributes {s}")))
}

fn validation_method_arg(s: &str) -> Result<geoforge_geometry::ValidationMethod> {
    match s.to_ascii_uppercase().as_str() {
        "ESRI" => Ok(geoforge_geometry::ValidationMethod::Esri),
        "OGC" => Ok(geoforge_geometry::ValidationMethod::Ogc),
        other => Err(Error::Unsupported(format!("validation_method {other}"))),
    }
}

fn check_geometry_tool(in_features: &str, out_table: &Path, method: &str) -> Result<()> {
    let method = validation_method_arg(method)?;
    let paths: Vec<PathBuf> = in_features
        .split(';')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .collect();
    let opened: Vec<Box<dyn FeatureClass>> = paths
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let inputs: Vec<geoforge_tools::CheckInput> = paths
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::CheckInput {
            source: catalog_path(p),
            features: f.as_ref(),
            has_spatial_index: match locate(p) {
                Location::Shapefile(shp) => shp.with_extension("sbn").exists(),
                Location::Geodatabase { .. } => true,
            },
        })
        .collect();
    let (rows, messages) = geoforge_tools::check_geometry(&inputs, method)?;
    for m in &messages {
        eprintln!("{m}");
    }
    if out_table
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("dbf"))
    {
        return Err(Error::Unsupported(
            "Check Geometry writes a .dbf table; geodatabase tables are not implemented yet".into(),
        ));
    }
    let fields = geoforge_tools::check_geometry_fields();
    let dbf_fields = fields
        .iter()
        .map(|f| geoforge_shapefile::dbf::DbfField::from_field(f.name.clone(), f))
        .collect::<Result<Vec<_>>>()?;
    let mut w = geoforge_shapefile::dbf::DbfWriter::create(
        out_table,
        dbf_fields,
        chrono::Local::now().date_naive(),
    )?;
    for r in &rows {
        w.push(&[
            Value::String(r.class.clone()),
            Value::Integer(r.feature_id as i32),
            Value::String(r.problem.clone()),
        ])?;
    }
    w.finish()?;
    let cpg = out_table.with_extension("cpg");
    std::fs::write(&cpg, "UTF-8").map_err(|e| Error::io(&cpg, e))?;
    println!("Wrote {} ({} problems)", out_table.display(), rows.len());
    Ok(())
}

fn split_tool(
    input: &Path,
    split_features: &Path,
    split_field: &str,
    out_workspace: &Path,
) -> Result<()> {
    if matches!(locate(out_workspace), Location::Geodatabase { .. }) {
        return Err(Error::Unsupported(
            "splitting into a geodatabase needs adding to an existing one (not implemented yet); use a folder".into(),
        ));
    }
    if !out_workspace.is_dir() {
        return Err(Error::Unsupported(format!(
            "ERROR 000732: Target Workspace: Dataset {} does not exist or is not supported",
            out_workspace.display()
        )));
    }
    let (fc, _) = open_feature_class(input)?;
    let (sf, _) = open_feature_class(split_features)?;
    for (value, out) in geoforge_tools::split(fc.as_ref(), sf.as_ref(), split_field)? {
        let name: String = value
            .chars()
            .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
            .collect();
        write_output(out, &out_workspace.join(format!("{name}.shp")), None)?;
    }
    Ok(())
}

fn merge_tool(inputs: &[PathBuf], output: &Path, add_source: &str) -> Result<()> {
    let add = match add_source.to_ascii_uppercase().as_str() {
        "NO_SOURCE_INFO" => false,
        "ADD_SOURCE_INFO" => true,
        other => return Err(Error::Unsupported(format!("add_source {other}"))),
    };
    let opened: Vec<Box<dyn FeatureClass>> = inputs
        .iter()
        .map(|p| open_feature_class(p).map(|(f, _)| f))
        .collect::<Result<_>>()?;
    let merge_inputs: Vec<geoforge_tools::PathInput> = inputs
        .iter()
        .zip(&opened)
        .map(|(p, f)| geoforge_tools::PathInput {
            source: catalog_path(p),
            features: f.as_ref(),
        })
        .collect();
    let out = geoforge_tools::merge(&merge_inputs, add)?;
    write_output(out, output, source_gdb(&inputs[0]).as_deref())
}

fn dissolve_tool(
    input: &Path,
    output: &Path,
    fields: &[String],
    statistics: Option<&str>,
    multi_part: &str,
) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let mut stats = Vec::new();
    for item in statistics
        .unwrap_or("")
        .split(';')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut words = item.split_whitespace();
        let (Some(field), Some(stat)) = (words.next(), words.next()) else {
            return Err(Error::Unsupported(format!(
                "statistic `{item}`: expected \"FIELD STATISTIC\""
            )));
        };
        let stat = geoforge_tools::Statistic::parse(stat)
            .ok_or_else(|| Error::Unsupported(format!("statistic type {stat}")))?;
        stats.push((field.to_owned(), stat));
    }
    let multi = match multi_part.to_ascii_uppercase().as_str() {
        "MULTI_PART" => true,
        "SINGLE_PART" => false,
        other => return Err(Error::Unsupported(format!("multi_part {other}"))),
    };
    let out = geoforge_tools::dissolve(fc.as_ref(), fields, &stats, multi)?;
    write_output(out, output, source_gdb(input).as_deref())
}

/// Parses an ArcGIS linear distance ("25", "25 Meters", "0.5 Kilometers") into the
/// units of `wkt` (the last `UNIT[...]` of a projected system).
fn parse_distance(text: &str, wkt: Option<&str>) -> Result<f64> {
    let mut words = text.split_whitespace();
    let value: f64 = words
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| Error::Unsupported(format!("invalid distance `{text}`")))?;
    let Some(unit) = words.next() else {
        return Ok(value);
    };
    let meters_per = |u: &str| -> Option<f64> {
        Some(match u.to_ascii_lowercase().trim_end_matches('s') {
            "meter" | "metre" => 1.0,
            "kilometer" | "kilometre" => 1000.0,
            "centimeter" => 0.01,
            "millimeter" => 0.001,
            "feet" | "foot" | "feetint" => 0.3048,
            "ussurveyfeet" | "foot_us" => 1200.0 / 3937.0,
            "yard" => 0.9144,
            "mile" | "milesint" => 1609.344,
            "nauticalmile" => 1852.0,
            _ => return None,
        })
    };
    let from = meters_per(unit)
        .ok_or_else(|| Error::Unsupported(format!("unknown distance unit `{unit}`")))?;
    // The data's linear unit: UNIT["Meter",1.0] at the end of a PROJCS.
    let data = wkt
        .and_then(|w| w.rsplit("UNIT[").next())
        .and_then(|u| u.split(',').nth(1))
        .and_then(|f| f.trim_end_matches([']', ' ']).parse::<f64>().ok())
        .unwrap_or(1.0);
    Ok(value * from / data)
}

fn buffer_tool(
    input: &Path,
    output: &Path,
    distance: &str,
    line_end: &str,
    dissolve: &str,
) -> Result<()> {
    let (fc, _) = open_feature_class(input)?;
    let d = parse_distance(distance, fc.spatial_reference_wkt())?;
    let line_end = match line_end.to_ascii_uppercase().as_str() {
        "ROUND" => geoforge_geometry::LineEnd::Round,
        "FLAT" => geoforge_geometry::LineEnd::Flat,
        other => return Err(Error::Unsupported(format!("line_end_type {other}"))),
    };
    let dissolve = match dissolve.to_ascii_uppercase().as_str() {
        "NONE" => Dissolve::None,
        "ALL" => Dissolve::All,
        other => {
            return Err(Error::Unsupported(format!(
                "dissolve_option {other} (LIST comes next)"
            )));
        }
    };
    let out = geoforge_tools::buffer(fc.as_ref(), d, line_end, dissolve)?;
    write_output(out, output, source_gdb(input).as_deref())
}
