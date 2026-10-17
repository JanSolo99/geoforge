//! Probe: for each vertex of an ArcGIS buffer ring, the nearest source vertex, geodesic distance and azimuth.
use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde_json::Value;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (src, ring): (Vec<[f64; 2]>, Vec<[f64; 2]>) = if args[1] == "ours" || args[1] == "ourspoly"
    {
        // ours <distance> <source vertices>: our buffer of the line through them
        let src: Vec<[f64; 2]> = serde_json::from_str(&args[3]).unwrap();
        let mut g = geoforge_core::Geometry::point(0.0, 0.0);
        g.parts = vec![0];
        if args[1] == "ourspoly" {
            g.kind = geoforge_core::GeometryKind::Polygon;
            g.xy = src
                .iter()
                .chain(std::iter::once(&src[0]))
                .copied()
                .collect();
        } else {
            g.kind = geoforge_core::GeometryKind::Polyline;
            g.xy = src.clone();
        }
        let o = geoforge_geometry::GeodesicOptions {
            distance: args[2].parse().unwrap(),
            line_end: geoforge_geometry::LineEnd::Round,
            side: geoforge_geometry::LineSide::Full,
        };
        let out = geoforge_geometry::buffer_geodesic(&g, &o).unwrap().unwrap();
        (src, out.xy.clone())
    } else {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
        let key = &args[2];
        let src: Vec<[f64; 2]> = serde_json::from_str(&args[3]).unwrap();
        let ring: Vec<[f64; 2]> = v[key][0]["parts"][0]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
            .collect();
        (src, ring)
    };
    let g = Geodesic::wgs84();
    let mut last = None;
    for (i, q) in ring.iter().enumerate() {
        let (best, d, az) = src
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let (d, az, _, _): (f64, f64, f64, f64) = g.inverse(s[1], s[0], q[1], q[0]);
                (k, d, az)
            })
            .fold((0, f64::MAX, 0.0), |a, b| if b.1 < a.1 { b } else { a });
        let step = last.map(|(_, la)| ((az - la + 540.0) % 360.0) - 180.0);
        println!(
            "{i:4} src{best} dist {d:9.4} az {az:9.4} step {}",
            step.map_or("-".into(), |s: f64| format!("{s:.4}"))
        );
        last = Some((best, az));
    }
}
