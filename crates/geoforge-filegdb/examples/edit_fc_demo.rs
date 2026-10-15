//! `edit_fc_demo <src.gdb> <dst.gdb> <feature class>`: copies a geodatabase and edits a point
//! feature class with fields WELL_ID (long) and DEPTH (double): two inserts, one moved
//! point, one delete. For checking the result in ArcGIS.
use geoforge_core::{Feature, Geometry, Value};
use geoforge_filegdb::{Geodatabase, TableEditor};

fn well(id: i32, x: f64, y: f64) -> Feature {
    Feature {
        oid: 0,
        geometry: Some(Geometry::point(x, y)),
        attributes: vec![Value::Integer(id), Value::Double(12.5)],
    }
}

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
    let base = Geodatabase::open(dst).unwrap().table_base(&a[3]).unwrap();
    let far = (510_000.0, 5_010_000.0);
    let mut ed = TableEditor::open(base).unwrap();
    println!("inserted {}", ed.insert(&well(900, far.0, far.1)).unwrap());
    println!(
        "inserted {}",
        ed.insert(&well(901, far.0 + 50.0, far.1 + 50.0)).unwrap()
    );
    ed.update(1, &well(1, far.0 - 30.0, far.1)).unwrap();
    ed.delete(2).unwrap();
    println!("rows now {}", ed.finish().unwrap());
}
