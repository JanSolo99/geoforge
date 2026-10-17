//! `cargo run -p geoforge-filegdb --example fds_demo -- out.gdb`: a geodatabase with a feature dataset.
use geoforge_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_filegdb::GeodatabaseWriter;

const UTM33: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn main() {
    let path = std::env::args().nth(1).expect("output .gdb path");
    let mut w = GeodatabaseWriter::create(&path).unwrap();
    w.create_feature_dataset("ds", Some(UTM33)).unwrap();
    let fields = [Field::new("k", FieldType::Integer)];
    let mut t = w
        .create_feature_class_in("ds", "pts", GeometryKind::Point, false, false, &fields)
        .unwrap();
    for i in 0..3 {
        t.write(&Feature {
            oid: 0,
            geometry: Some(Geometry::point(
                500_000.0 + f64::from(i) * 10.0,
                5_000_000.0,
            )),
            attributes: vec![Value::Integer(i)],
        })
        .unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();
}
