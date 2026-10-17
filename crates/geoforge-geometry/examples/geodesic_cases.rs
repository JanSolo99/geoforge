//! Runs every case of a geodesic buffer reference file and reports how close our rings are to ArcGIS's.
use geoforge_core::{Geometry, GeometryKind};
use geoforge_geometry::{GeodesicOptions, LineEnd, LineSide, buffer_geodesic};
use geographiclib_rs::{Geodesic, InverseGeodesic, PolygonArea, Winding};
use serde_json::Value;

type Pt = [f64; 2];

fn rings_of(parts: &Value) -> Vec<Vec<Pt>> {
    // A part is a list of points, rings separated by null.
    let mut out = Vec::new();
    for part in parts.as_array().unwrap() {
        let mut cur: Vec<Pt> = Vec::new();
        for p in part.as_array().unwrap() {
            if p.is_null() {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            } else {
                cur.push([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]);
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

fn area(g: &Geodesic, ring: &[Pt]) -> f64 {
    let mut pa = PolygonArea::new(g, Winding::CounterClockwise);
    for p in ring {
        pa.add_point(p[1], p[0]);
    }
    pa.compute(true).1
}

fn main() {
    let cases: Value =
        serde_json::from_str(&std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap())
            .unwrap();
    let g = Geodesic::wgs84();
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let kind = c["kind"].as_str().unwrap();
        let src = rings_of(&c["parts"]);
        let mut geom = Geometry::point(0.0, 0.0);
        geom.xy.clear();
        geom.kind = match kind {
            "point" | "multipoint" => GeometryKind::Multipoint,
            "line" => GeometryKind::Polyline,
            _ => GeometryKind::Polygon,
        };
        for r in &src {
            if matches!(kind, "line" | "polygon") {
                geom.parts.push(geom.xy.len() as u32);
            }
            geom.xy.extend(r.iter().copied());
        }
        let dist = c["dist"].as_f64().unwrap();
        let opts = GeodesicOptions {
            distance: dist,
            line_end: if c["line_end"] == "FLAT" {
                LineEnd::Flat
            } else {
                LineEnd::Round
            },
            side: match c["line_side"].as_str().unwrap() {
                "LEFT" => LineSide::Left,
                "RIGHT" => LineSide::Right,
                "OUTSIDE_ONLY" => LineSide::OutsideOnly,
                _ => LineSide::Full,
            },
        };
        let theirs: Vec<Vec<Pt>> = rings_of(&c["result"]);
        let ours = match buffer_geodesic(&geom, &opts) {
            Ok(Some(o)) => o,
            Ok(None) => {
                println!("{name}: empty (ArcGIS {} rings)", theirs.len());
                continue;
            }
            Err(e) => {
                println!("{name}: ERROR {e}");
                continue;
            }
        };
        let o_rings: Vec<Vec<Pt>> = ours.part_ranges().map(|r| ours.xy[r].to_vec()).collect();
        // Furthest any vertex of `a` is from the boundary of `b` (metres, in a local frame at the vertex).
        let far = |a: &[Vec<Pt>], b: &[Vec<Pt>]| {
            let mut worst = 0.0_f64;
            for p in a.iter().flatten() {
                let m_lon: f64 = {
                    let (s, _, _, _): (f64, f64, f64, f64) =
                        g.inverse(p[1], p[0], p[1], p[0] + 1e-4);
                    s / 1e-4
                };
                let m_lat: f64 = {
                    let (s, _, _, _): (f64, f64, f64, f64) =
                        g.inverse(p[1], p[0], p[1] + 1e-4, p[0]);
                    s / 1e-4
                };
                let to = |q: &Pt| [(q[0] - p[0]) * m_lon, (q[1] - p[1]) * m_lat];
                let mut best = f64::MAX;
                for ring in b {
                    for w in ring.windows(2) {
                        let (a0, a1) = (to(&w[0]), to(&w[1]));
                        let (dx, dy) = (a1[0] - a0[0], a1[1] - a0[1]);
                        let l2 = dx * dx + dy * dy;
                        let t = if l2 == 0.0 {
                            0.0
                        } else {
                            ((-a0[0]) * dx + (-a0[1]) * dy) / l2
                        }
                        .clamp(0.0, 1.0);
                        best = best.min((a0[0] + t * dx).hypot(a0[1] + t * dy));
                    }
                }
                worst = worst.max(best);
            }
            worst
        };
        let total = |rs: &[Vec<Pt>]| rs.iter().map(|r| area(&g, r)).sum::<f64>();
        let (ao, at) = (total(&o_rings), total(&theirs));
        println!(
            "{name}: rings {}/{} vertices {}/{} area {ao:.2}/{at:.2} (rel {:.1e}) offsets {:.3}/{:.3} m",
            o_rings.len(),
            theirs.len(),
            o_rings.iter().map(Vec::len).sum::<usize>(),
            theirs.iter().map(Vec::len).sum::<usize>(),
            ((ao - at) / at.abs().max(1e-9)).abs(),
            far(&o_rings, &theirs),
            far(&theirs, &o_rings)
        );
    }
}
