//! Geodesic buffers of geographic data against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/geodesic_buffers.json: each case is a source geometry in WGS 84, a distance
//! in metres and the rings ArcGIS's Buffer gave, made through the bridge).
//!
//! Points, and lines and polygons up to 50 km, equal ArcGIS's vertex for vertex or lie within a few
//! centimetres of its boundary; beyond 50 km ArcGIS builds lines with a 100 m chord deviation in a way
//! that is only matched to that deviation.

use std::path::{Path, PathBuf};

use geographiclib_rs::{Geodesic, InverseGeodesic};
use serde_json::Value;
use unigis_core::{Geometry, GeometryKind};
use unigis_geometry::{GeodesicOptions, LineEnd, LineSide, buffer_geodesic};

type Pt = [f64; 2];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Rings of a result: parts are lists of points with `null` between rings.
fn rings_of(parts: &Value) -> Vec<Vec<Pt>> {
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

/// Furthest any vertex of `a` is from the boundary of `b`, in metres (measured in a local frame).
fn furthest(g: &Geodesic, a: &[Vec<Pt>], b: &[Vec<Pt>]) -> f64 {
    let mut worst = 0.0_f64;
    for p in a.iter().flatten() {
        let scale = |dlon: f64, dlat: f64| -> f64 {
            let (s, _, _, _): (f64, f64, f64, f64) =
                g.inverse(p[1], p[0], p[1] + dlat, p[0] + dlon);
            s
        };
        let (m_lon, m_lat) = (scale(1e-4, 0.0) / 1e-4, scale(0.0, 1e-4) / 1e-4);
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
}

fn geometry(kind: &str, rings: &[Vec<Pt>]) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.xy.clear();
    g.kind = match kind {
        "point" | "multipoint" => GeometryKind::Multipoint,
        "line" => GeometryKind::Polyline,
        _ => GeometryKind::Polygon,
    };
    for r in rings {
        if matches!(kind, "line" | "polygon") {
            g.parts.push(g.xy.len() as u32);
        }
        g.xy.extend(r.iter().copied());
    }
    g
}

/// (case, the furthest a vertex of ours is from ArcGIS's boundary and back, in metres, and whether the
/// vertex counts must be equal).
const EXPECT: &[(&str, f64, bool)] = &[
    ("point_100", 0.001, true),
    ("point_1000", 0.001, true),
    ("point_5000", 0.001, true),
    ("point_30000", 0.001, true),
    ("point_80000", 0.001, true),
    ("point_300000", 0.001, true),
    ("point_lat65_2000", 0.001, true),
    ("multipoint_300", 0.001, true),
    ("line_50", 0.001, true),
    ("line_500", 0.001, true),
    ("line_5000", 0.001, true),
    ("line_flat_500", 0.001, true),
    ("line_left_500", 0.001, true),
    ("line_right_500", 0.001, true),
    ("line_left_flat_500", 0.001, true),
    ("line_closed_300", 0.02, true),
    ("multiline_300", 0.1, false),
    ("polygon_300", 0.05, false),
    ("polygon_5000", 0.05, false),
    ("polygon_hole_100", 0.02, false),
    ("polygon_neg_100", 0.001, true),
    ("polygon_outside_300", 0.03, true),
    ("polygon_lat65_1000", 0.1, false),
    // A sharp bend at 65 N: ArcGIS resolves the inside of the bend with extra vertices.
    ("line_lat65_2000", 1.0, false),
    // Beyond 50 km ArcGIS's line arcs are only matched to their 100 m deviation.
    ("line_80000", 200.0, false),
];

#[test]
fn geodesic_buffers_equal_arcgis() {
    let cases: Value = serde_json::from_slice(
        &std::fs::read(root().join("testdata/arcgis-reference/geodesic_buffers.json")).unwrap(),
    )
    .unwrap();
    let g = Geodesic::wgs84();
    let mut checked = 0;
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let Some(&(_, tolerance, same_count)) = EXPECT.iter().find(|e| e.0 == name) else {
            panic!("case {name} has no expectation");
        };
        let kind = c["kind"].as_str().unwrap();
        let geom = geometry(kind, &rings_of(&c["parts"]));
        let opts = GeodesicOptions {
            distance: c["dist"].as_f64().unwrap(),
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
        let theirs = rings_of(&c["result"]);
        let ours = buffer_geodesic(&geom, &opts)
            .unwrap()
            .unwrap_or_else(|| panic!("{name}: empty"));
        let ours: Vec<Vec<Pt>> = ours.part_ranges().map(|r| ours.xy[r].to_vec()).collect();
        assert_eq!(ours.len(), theirs.len(), "{name}: rings");
        let (a, b) = (furthest(&g, &ours, &theirs), furthest(&g, &theirs, &ours));
        assert!(
            a <= tolerance && b <= tolerance,
            "{name}: vertices off the other boundary by {a:.4} / {b:.4} m (allowed {tolerance})"
        );
        if same_count {
            let n = |r: &[Vec<Pt>]| r.iter().map(Vec::len).sum::<usize>();
            assert_eq!(n(&ours), n(&theirs), "{name}: vertex count");
        }
        checked += 1;
    }
    assert_eq!(checked, EXPECT.len());
}

#[test]
fn point_buffers_are_geodesic_circles() {
    let g = Geodesic::wgs84();
    for (lon, lat, r) in [
        (10.0, 50.0, 1000.0),
        (-120.0, 0.0, 250.0),
        (25.0, 78.0, 12_000.0),
    ] {
        let mut p = Geometry::point(lon, lat);
        p.xy = vec![[lon, lat]];
        let o = buffer_geodesic(
            &p,
            &GeodesicOptions {
                distance: r,
                line_end: LineEnd::Round,
                side: LineSide::Full,
            },
        )
        .unwrap()
        .unwrap();
        let ring = &o.xy[..o.parts.get(1).map_or(o.xy.len(), |p| *p as usize)];
        for v in ring {
            let (s, _, _, _): (f64, f64, f64, f64) = g.inverse(lat, lon, v[1], v[0]);
            assert!((s - r).abs() < 1e-4, "{s} vs {r}");
        }
    }
    // A buffer over the antimeridian is refused rather than drawn wrongly.
    let mut p = Geometry::point(179.9999, 0.0);
    p.xy = vec![[179.9999, 0.0]];
    assert!(
        buffer_geodesic(
            &p,
            &GeodesicOptions {
                distance: 5000.0,
                line_end: LineEnd::Round,
                side: LineSide::Full
            }
        )
        .is_err()
    );
}
