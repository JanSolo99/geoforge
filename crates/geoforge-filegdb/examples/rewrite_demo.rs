//! `rewrite_demo <src.gdb> <dst.gdb>`: copies a geodatabase, adds a field EXTRA to feature class
//! `wells` and drops its row 3 and field DEPTH, for checking the result in ArcGIS.
use geoforge_core::{Feature, Field, FieldType, Value};
use geoforge_filegdb::{Geodatabase, rewrite_table};

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
    let gdb = Geodatabase::open(dst).unwrap();
    let base = gdb.table_base("wells").unwrap();
    let t = gdb.open_table("wells").unwrap();
    // Fields: WELL_ID, DEPTH -> WELL_ID, EXTRA
    let fields = vec![
        t.fields()[0].clone(),
        Field::new("EXTRA", FieldType::Integer),
    ];
    let rows: Vec<(Option<u64>, Feature)> = t
        .features()
        .map(|f| f.unwrap())
        .filter(|f| f.oid != 3)
        .map(|mut f| {
            f.attributes = vec![f.attributes[0].clone(), Value::Integer(f.oid as i32 * 10)];
            (Some(f.oid as u64), f)
        })
        .collect();
    println!("rows {}", rewrite_table(base, &fields, &rows).unwrap());
}
