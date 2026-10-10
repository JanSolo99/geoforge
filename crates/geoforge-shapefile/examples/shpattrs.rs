//! Prints FID, first attributes, part count and vertex count of each feature.
fn main() {
    let r = geoforge_shapefile::ShapefileReader::open(std::env::args().nth(1).unwrap()).unwrap();
    for f in r.features() {
        let f = f.unwrap();
        let g = f.geometry.as_ref();
        println!(
            "{} {:?} parts={} pts={}",
            f.oid,
            &f.attributes[..f.attributes.len().min(2)],
            g.map_or(0, |g| g.num_parts()),
            g.map_or(0, |g| g.xy.len())
        );
    }
}
