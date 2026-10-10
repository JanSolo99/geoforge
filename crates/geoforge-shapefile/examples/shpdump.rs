//! `cargo run -p geoforge-shapefile --example shpdump -- <file.shp>`
fn main() {
    let r = geoforge_shapefile::ShapefileReader::open(std::env::args().nth(1).unwrap()).unwrap();
    println!("type {:?} extent {:?}", r.shape_type(), r.extent());
    for f in r.features() {
        let f = f.unwrap();
        println!(
            "{} {:?}",
            f.oid,
            f.geometry
                .map(|g| (g.kind, g.parts, g.xy.len(), g.xy.first().copied()))
        );
    }
}
