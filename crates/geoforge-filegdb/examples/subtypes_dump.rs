//! `cargo run -p geoforge-filegdb --example subtypes_dump -- X.gdb table`: the subtypes the reader finds.
use geoforge_filegdb::Geodatabase;

fn main() {
    let mut a = std::env::args().skip(1);
    let (gdb, table) = (a.next().unwrap(), a.next().unwrap());
    let g = Geodatabase::open(&gdb).unwrap();
    println!("{:?}", g.entries());
    let t = g.open_table(&table).unwrap();
    println!("{:#?}", t.subtypes());
    for f in t.all_fields() {
        println!("{} {:?} default={:?}", f.name, f.field_type, f.default);
    }
}
