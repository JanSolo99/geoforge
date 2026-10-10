//! `cargo run -p geoforge-filegdb --example dump -- <x.gdb> [layer]`
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let gdb = geoforge_filegdb::Geodatabase::open(&args[1]).unwrap();
    for e in gdb.entries() {
        let t = gdb.open_table(&e.name);
        match t {
            Ok(t) => println!(
                "{} (a{:08x}) fd={:?} kind={:?} z={} m={} rows={}",
                e.name,
                e.table_id,
                e.feature_dataset,
                t.geometry_kind(),
                t.has_z(),
                t.has_m(),
                t.row_count()
            ),
            Err(err) => println!("{}: ERROR {err}", e.name),
        }
    }
    if let Some(layer) = args.get(2) {
        let t = gdb.open_table(layer).unwrap();
        for f in t.fields() {
            println!("  field {:?}", f);
        }
        for f in t.features().take(5) {
            println!("  {:?}", f);
        }
    }
}
