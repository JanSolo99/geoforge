//! `extent <table> [xmin ymin xmax ymax]`: indexed vs full-scan extent query.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = geoforge_filegdb::Table::open(&a[1]).unwrap();
    let v: Vec<f64> = a[2..].iter().map(|s| s.parse().unwrap()).collect();
    let (xmin, ymin, xmax, ymax) = if v.len() == 4 {
        (v[0], v[1], v[2], v[3])
    } else {
        (500100.0, 4500100.0, 500400.0, 4500400.0)
    };
    let env = geoforge_core::Envelope {
        xmin,
        ymin,
        xmax,
        ymax,
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
