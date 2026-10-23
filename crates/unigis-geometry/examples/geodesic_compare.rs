//! Compares geodesic buffers with ArcGIS's (a JSON file of {pt, ln, pg} rows made by the bridge).
use geographiclib_rs::{Geodesic, InverseGeodesic, PolygonArea, Winding};
use serde_json::Value;
use unigis_core::{Geometry, GeometryKind};
use unigis_geometry::{GeodesicOptions, LineEnd, LineSide, buffer_geodesic};

fn poly(ring: &[[f64; 2]]) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polygon;
    g.parts = vec![0];
    g.xy = ring.to_vec();
    g
}

fn line(pts: &[[f64; 2]]) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polyline;
    g.parts = vec![0];
    g.xy = pts.to_vec();
    g
}

fn area(g: &Geodesic, pts: &[[f64; 2]]) -> f64 {
    let mut pa = PolygonArea::new(g, Winding::CounterClockwise);
    for p in pts {
        pa.add_point(p[1], p[0]);
    }
    let (_, a, _) = pa.compute(true);
    a.abs()
}

fn report(name: &str, ours: &Geometry, theirs: &[[f64; 2]]) {
    let g = Geodesic::wgs84();
    let ring: Vec<[f64; 2]> = ours.xy[ours.parts.first().map_or(0, |p| *p as usize)
        ..ours.parts.get(1).map_or(ours.xy.len(), |p| *p as usize)]
        .to_vec();
    // Furthest our vertex is from any of theirs, and the reverse.
    let far = |a: &[[f64; 2]], b: &[[f64; 2]]| {
        a.iter()
            .map(|p| {
                b.iter()
                    .map(|q| {
                        let (s, _, _, _): (f64, f64, f64, f64) = g.inverse(p[1], p[0], q[1], q[0]);
                        s
                    })
                    .fold(f64::MAX, f64::min)
            })
            .fold(0.0_f64, f64::max)
    };
    if std::env::var("DETAIL").is_ok() {
        for (i, p) in ring.iter().enumerate() {
            let d = theirs
                .iter()
                .map(|q| {
                    let (s, _, _, _): (f64, f64, f64, f64) = g.inverse(p[1], p[0], q[1], q[0]);
                    s
                })
                .fold(f64::MAX, f64::min);
            if d > 0.01 {
                println!(
                    "  ours[{i}] {:?} is {d:.3} m from the nearest ArcGIS vertex",
                    p
                );
            }
        }
    }
    println!(
        "{name}: ours {} vertices, ArcGIS {}; area ours {:.4} theirs {:.4}; vertex offsets {:.4} m / {:.4} m",
        ring.len(),
        theirs.len(),
        area(&g, &ring),
        area(&g, theirs),
        far(&ring, theirs),
        far(theirs, &ring)
    );
}

fn main() {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap())
            .unwrap();
    let pts = |row: &Value, part: usize| -> Vec<[f64; 2]> {
        row["parts"][part]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
            .collect()
    };
    let opts = |d: f64| GeodesicOptions {
        distance: d,
        line_end: LineEnd::Round,
        side: LineSide::Full,
    };
    for (i, c) in [[10.0, 50.0], [-120.0, 0.0], [25.0, 78.0]]
        .iter()
        .enumerate()
    {
        let mut p = Geometry::point(c[0], c[1]);
        p.xy = vec![*c];
        let out = buffer_geodesic(&p, &opts(1000.0)).unwrap().unwrap();
        report(&format!("point {i}"), &out, &pts(&v["pt"][i], 0));
    }
    if std::env::var("RING").is_ok() {
        let mut p = Geometry::point(10.0, 50.0);
        p.xy = vec![[10.0, 50.0]];
        let out = buffer_geodesic(&p, &opts(1000.0)).unwrap().unwrap();
        let g = Geodesic::wgs84();
        for (i, q) in out
            .xy
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < 3 || *i > 219)
        {
            let (s, az, _, _): (f64, f64, f64, f64) = g.inverse(50.0, 10.0, q[1], q[0]);
            println!("  [{i}] {:.9} {:.9} dist {s:.3} az {az:.4}", q[0], q[1]);
        }
    }
    let l = line(&[[10.0, 50.0], [10.02, 50.01], [10.04, 50.0]]);
    report(
        "line 500 m",
        &buffer_geodesic(&l, &opts(500.0)).unwrap().unwrap(),
        &pts(&v["ln"][0], 0),
    );
    let g = poly(&[
        [10.0, 50.0],
        [10.0, 50.01],
        [10.02, 50.01],
        [10.02, 50.0],
        [10.0, 50.0],
    ]);
    report(
        "polygon 300 m",
        &buffer_geodesic(&g, &opts(300.0)).unwrap().unwrap(),
        &pts(&v["pg"][0], 0),
    );
}
