//! Times GeoPackage writing: `cargo run --release --example bench_gpkg -- out.gpkg [features]`.
use std::time::Instant;

use geoforge_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_gpkg::write::{NewTable, SpatialRef, write_table};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = std::path::PathBuf::from(args.next().expect("output path"));
    let n: i64 = args.next().map_or(1_000_000, |s| s.parse().unwrap());
    let fields = vec![
        Field::new("id", FieldType::Integer),
        Field::new("value", FieldType::Double),
        {
            let mut f = Field::new("name", FieldType::String);
            f.length = 40;
            f
        },
    ];
    let features: Vec<Feature> = (0..n)
        .map(|k| Feature {
            oid: k,
            geometry: Some(Geometry::point(
                500000.0 + (k % 1000) as f64 * 1.37,
                5000000.0 + (k / 1000) as f64 * 2.11,
            )),
            attributes: vec![
                Value::Integer(k as i32),
                Value::Double(k as f64 * 0.25),
                Value::String(format!("feature {k}")),
            ],
        })
        .collect();
    let sr = SpatialRef {
        srs_id: 32633,
        srs_name: "WGS_1984_UTM_Zone_33N".into(),
        organization: "EPSG".into(),
        organization_coordsys_id: 32633,
        definition: "undefined".into(),
        description: None,
        definition_12_063: "undefined".into(),
    };
    std::fs::remove_file(&path).ok();
    let start = Instant::now();
    write_table(
        &path,
        &NewTable {
            name: "points",
            geometry: Some((GeometryKind::Point, false, false)),
            fields: &fields,
            spatial_ref: Some(&sr),
            xy_domain: Some((-5120900.0, -9998100.0, 0.0001)),
            features: &features,
        },
    )
    .unwrap();
    let t = start.elapsed();
    let size = std::fs::metadata(&path).unwrap().len();
    println!(
        "{n} points: {t:.2?} ({:.0} features/s), {:.1} MB",
        n as f64 / t.as_secs_f64(),
        size as f64 / 1e6
    );
}
