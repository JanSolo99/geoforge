//! `edit_demo <src.gdb> <dst.gdb> <table file base, e.g. a0000000b>`: copies a geodatabase and
//! applies a fixed series of edits (two inserts, an update, two deletes) to a table with
//! fields ID (long) and NAME (text 20), for checking the result in ArcGIS.
use unigis_core::{Feature, Value};
use unigis_filegdb::TableEditor;

fn row(id: i32, name: &str) -> Feature {
    Feature {
        oid: 0,
        geometry: None,
        attributes: vec![Value::Integer(id), Value::String(name.into())],
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
    let mut ed = TableEditor::open(dst.join(&a[3])).unwrap();
    println!(
        "inserted OBJECTID {}",
        ed.insert(&row(1000, "added a")).unwrap()
    );
    println!(
        "inserted OBJECTID {}",
        ed.insert(&row(1001, "added b")).unwrap()
    );
    ed.update(5, &row(5, "longer replacement")).unwrap();
    ed.delete(10).unwrap();
    ed.delete(11).unwrap();
    println!("rows now {}", ed.finish().unwrap());
}
