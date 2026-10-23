fn main() {
    let a: Vec<String> = std::env::args().collect();
    let t = unigis_filegdb::Table::open(&a[1]).unwrap();
    let d = t.geometry_def().unwrap();
    println!(
        "grid {:?} origin {} {} scale {} kind {:?}",
        d.grid_sizes,
        d.x_origin,
        d.y_origin,
        d.xy_scale,
        t.geometry_kind()
    );
    for f in t
        .features()
        .take(a.get(2).map_or(3, |n| n.parse().unwrap()))
    {
        let f = f.unwrap();
        if let Some(g) = f.geometry {
            let e = g.envelope();
            println!(
                "oid {} env {} {} {} {}",
                f.oid, e.xmin, e.ymin, e.xmax, e.ymax
            );
        }
    }
}
