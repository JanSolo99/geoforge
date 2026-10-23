//! Probe: how far (geodesically, WGS 84) the vertices of ArcGIS's buffers are from their source.
use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde_json::Value;

fn main() {
    let path = std::env::args().nth(1).expect("buffers.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let g = Geodesic::wgs84();
    let centers = [(10.0, 50.0), (-120.0, 0.0), (25.0, 78.0)];
    for (i, row) in v["pt"].as_array().unwrap().iter().enumerate() {
        let ring = row["parts"][0].as_array().unwrap();
        let (cx, cy) = centers[i];
        let mut d = Vec::new();
        let mut az = Vec::new();
        for p in ring {
            let (x, y) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
            let (s12, azi1, _a2, _): (f64, f64, f64, f64) = g.inverse(cy, cx, y, x);
            d.push(s12);
            az.push(azi1);
        }
        let (mn, mx) = d
            .iter()
            .fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        println!(
            "point {i}: {} vertices, distance {mn:.6}..{mx:.6}",
            ring.len()
        );
        println!(
            "  azimuth steps: {:?}",
            az.windows(2)
                .take(6)
                .map(|w| (w[1] - w[0] + 540.0) % 360.0 - 180.0)
                .collect::<Vec<_>>()
        );
        println!(
            "  first azimuths: {:?} last: {:?}",
            &az[..3],
            &az[az.len() - 2..]
        );
        println!("  first vertex {:?}", ring[0]);
    }
}
