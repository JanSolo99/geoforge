//! `gf`: GeoForge geoprocessing tools from the command line. Tool names and
//! behavior follow their ArcGIS Pro counterparts.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use geoforge_core::{FieldType, Result};
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

fn describe(path: &std::path::Path) -> Result<()> {
    let r = ShapefileReader::open(path)?;
    let st = r.shape_type();
    let py = |b: bool| if b { "True" } else { "False" };
    println!("dataType:         ShapeFile");
    println!("shapeType:        {}", st.esri_name());
    println!("hasZ:             {}", py(st.has_z()));
    println!("hasM:             {}", py(st.has_m()));
    println!("OIDFieldName:     FID");
    println!("shapeFieldName:   Shape");
    println!("featureCount:     {}", r.features().count());
    let e = r.extent();
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
    let sr = r
        .spatial_reference_wkt()
        .map_or("Unknown".to_owned(), |wkt| {
            wkt.split('"').nth(1).unwrap_or("Unknown").to_owned()
        });
    println!("spatialReference: {sr}");
    println!("fields:");
    println!("  {:<12} {:<12}", "FID", "OID");
    println!("  {:<12} {:<12}", "Shape", "Geometry");
    for f in r.fields() {
        let detail = match f.field_type {
            FieldType::String => format!("length {}", f.length),
            FieldType::Date => String::new(),
            _ => format!("precision {}, scale {}", f.precision, f.scale),
        };
        println!(
            "  {:<12} {:<12} {detail}",
            f.name,
            esri_type_name(f.field_type)
        );
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

fn copy_features(input: &std::path::Path, output: &std::path::Path) -> Result<()> {
    let r = ShapefileReader::open(input)?;
    let st = r.shape_type();
    let Some(kind) = st.kind() else {
        return Err(geoforge_core::Error::Unsupported(
            "input has shape type Null; there is no geometry type to copy".into(),
        ));
    };
    let mut w = ShapefileWriter::create(
        output,
        kind,
        st.has_z(),
        st.has_m(),
        r.fields(),
        r.spatial_reference_wkt(),
    )?;
    let mut n = 0;
    for feature in r.features() {
        w.write(&feature?)?;
        n += 1;
    }
    w.finish()?;
    println!("Copied {n} features to {}", output.display());
    Ok(())
}
