//! `add_to_gdb <src.gdb> <dst.gdb>`: copies a geodatabase and adds a table `added_tab`
//! (NAME text 30, N long, 3 rows) and a point feature class `added_fc` (WELL_ID long,
//! UTM 33N, 3 points) to it, for checking the result in ArcGIS.
use geoforge_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_filegdb::GeodatabaseWriter;

const UTM33: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (src, dst) = (std::path::Path::new(&a[1]), std::path::Path::new(&a[2]));
    std::fs::remove_dir_all(dst).ok();
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        if e.file_name().to_string_lossy().ends_with(".lock") {
            continue;
        }
        std::fs::copy(e.path(), dst.join(e.file_name())).unwrap();
    }
    let mut w = GeodatabaseWriter::open_existing(dst).unwrap();
    let mut name = Field::new("NAME", FieldType::String);
    name.length = 30;
    let fields = [name, Field::new("N", FieldType::Integer)];
    let mut t = w
        .create_table("added_tab", None, false, false, &fields, None)
        .unwrap();
    for i in 0..3 {
        t.write(&Feature {
            oid: 0,
            geometry: None,
            attributes: vec![Value::String(format!("row {i}")), Value::Integer(i * 10)],
        })
        .unwrap();
    }
    w.finish_table(t).unwrap();
    let fields = [Field::new("WELL_ID", FieldType::Integer)];
    let mut t = w
        .create_table(
            "added_fc",
            Some(GeometryKind::Point),
            false,
            false,
            &fields,
            Some(UTM33),
        )
        .unwrap();
    for i in 0..3 {
        t.write(&Feature {
            oid: 0,
            geometry: Some(Geometry::point(
                500_000.0 + f64::from(i) * 100.0,
                5_000_000.0,
            )),
            attributes: vec![Value::Integer(i + 1)],
        })
        .unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();
    println!("added");
}
