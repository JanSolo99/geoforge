//! Prints every feature's attributes, part count and area/length.
fn main() {
    let r = geoforge_shapefile::ShapefileReader::open(std::env::args().nth(1).unwrap()).unwrap();
    let limit: usize = std::env::args().nth(2).map_or(12, |n| n.parse().unwrap());
    let mut n = 0;
    for f in r.features() {
        let f = f.unwrap();
        n += 1;
        if n > limit {
            continue;
        }
        let g = f.geometry.as_ref();
        let attrs: Vec<String> = f
            .attributes
            .iter()
            .map(|v| match v {
                geoforge_core::Value::Integer(i) => i.to_string(),
                geoforge_core::Value::SmallInteger(i) => i.to_string(),
                geoforge_core::Value::Double(d) => format!("{d}"),
                geoforge_core::Value::String(s) => s.clone(),
                v => format!("{v:?}"),
            })
            .collect();
        println!(
            "{:>3} [{}] parts={} pts={}",
            f.oid,
            attrs.join(", "),
            g.map_or(0, |g| g.num_parts()),
            g.map_or(0, |g| g.xy.len())
        );
    }
    println!("total {n}");
}
