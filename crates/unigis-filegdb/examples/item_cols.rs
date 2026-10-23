use unigis_core::Value;
fn main() {
    let t = unigis_filegdb::Table::open(std::env::args().nth(1).unwrap()).unwrap();
    let names: Vec<_> = t.fields().iter().map(|f| f.name.clone()).collect();
    for f in t.features() {
        let f = f.unwrap();
        let cols: Vec<String> = names
            .iter()
            .zip(&f.attributes)
            .map(|(n, v)| match v {
                Value::Xml(_) => format!("{n}=<xml>"),
                v => format!("{n}={v:?}"),
            })
            .collect();
        println!("{}: {}", f.oid, cols.join(" | "));
    }
}
