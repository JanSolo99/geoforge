//! Times an extent query with and without the spatial index.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = geoforge_filegdb::Table::open(&a[1]).unwrap();
    let env = geoforge_core::Envelope {
        xmin: 500100.0,
        ymin: 4500100.0,
        xmax: 500400.0,
        ymax: 4500400.0,
        z: None,
        m: None,
    };
    let t0 = std::time::Instant::now();
    let hits = t.features_in_extent(&env).unwrap().len();
    let indexed = t0.elapsed();
    let t1 = std::time::Instant::now();
    let scan = t
        .features()
        .filter(|f| {
            f.as_ref().unwrap().geometry.as_ref().is_some_and(|g| {
                let e = g.envelope();
                e.xmin <= env.xmax && e.xmax >= env.xmin && e.ymin <= env.ymax && e.ymax >= env.ymin
            })
        })
        .count();
    println!(
        "{hits} hits via index in {indexed:?}; full scan found {scan} in {:?}",
        t1.elapsed()
    );
}
