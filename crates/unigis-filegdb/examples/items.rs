//! `cargo run -p unigis-filegdb --example items -- <x.gdb> [name-filter]`: dumps GDB_Items.
use unigis_core::Value;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = std::path::Path::new(&a[1]);
    let cat = unigis_filegdb::Table::open(dir.join("a00000001")).unwrap();
    let id = cat
        .features()
        .map(Result::unwrap)
        .find(|f| f.attributes[0] == Value::String("GDB_Items".into()))
        .unwrap()
        .oid;
    let t = unigis_filegdb::Table::open(dir.join(format!("a{id:08x}"))).unwrap();
    let names: Vec<_> = t.fields().iter().map(|f| f.name.clone()).collect();
    for f in t.features().map(Result::unwrap) {
        let get = |n: &str| f.attributes[names.iter().position(|x| x == n).unwrap()].clone();
        let name = format!("{:?}", get("Name"));
        if a.get(2).is_some_and(|flt| !name.contains(flt.as_str())) {
            continue;
        }
        println!("== {name} type={:?} path={:?}", get("Type"), get("Path"));
        if let Value::Xml(x) = get("Definition") {
            println!("{}", &x[..]);
        }
    }
}
