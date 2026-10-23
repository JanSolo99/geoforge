//! Writes the index of the first run of `cargo test ... GF_BIG_GDB` for inspection:
//! `cargo run -p unigis-filegdb --example atx_dump -- <ArcGIS gdb> <out dir>` writes
//! our `.atx` for table `t` (fields L, S, D) so it can be compared with ArcGIS's.
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let table = Geodatabase::open(&a[1]).unwrap().open_table("t").unwrap();
    let fields: Vec<_> = table
        .fields()
        .iter()
        .filter(|f| !f.name.eq_ignore_ascii_case("OBJECTID"))
        .cloned()
        .collect();
    let out = std::path::Path::new(&a[2]).join("ours.gdb");
    std::fs::remove_dir_all(&out).ok();
    std::fs::create_dir_all(&a[2]).unwrap();
    let mut w = GeodatabaseWriter::create(&out).unwrap();
    let mut t = w
        .create_table("t", None, false, false, &fields, None)
        .unwrap();
    for (name, field) in [("i_l", "L"), ("i_s", "S"), ("i_d", "D")] {
        t.add_attribute_index(name, field).unwrap();
    }
    for r in table.features() {
        t.write(&r.unwrap()).unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();
}
