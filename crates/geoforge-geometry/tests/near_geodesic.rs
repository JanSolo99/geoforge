//! Geodesic Near against ArcGIS Pro 3.7 (testdata/arcgis-reference/near_geodesic.json: 9 input/near
//! combinations of points, lines and polygons, with and without a 500 km search radius).

use std::path::Path;

use geoforge_core::{Geometry, GeometryKind};
use geoforge_geometry::{azimuth, nearest_geodesic};
use serde_json::Value;

fn coords(v: &Value) -> Vec<[f64; 2]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
        .collect()
}

fn geometries(set: &Value, kind: &str) -> Vec<Geometry> {
    set.as_array()
        .unwrap()
        .iter()
        .map(|g| match kind {
            "pts" => Geometry::point(g[0].as_f64().unwrap(), g[1].as_f64().unwrap()),
            _ => {
                let mut xy = coords(g);
                let mut out = Geometry::point(0.0, 0.0);
                out.kind = if kind == "lines" {
                    GeometryKind::Polyline
                } else {
                    GeometryKind::Polygon
                };
                if kind == "polys" {
                    xy.push(xy[0]);
                }
                out.parts = vec![0];
                out.xy = xy;
                out
            }
        })
        .collect()
}

#[test]
fn geodesic_near_equals_arcgis() {
    let reference: Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/arcgis-reference/near_geodesic.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut worst: f64 = 0.0;
    let mut failures = Vec::new();
    for c in reference["cases"].as_array().unwrap() {
        let (ik, nk) = (c["input"].as_str().unwrap(), c["near"].as_str().unwrap());
        let inputs = geometries(&reference["inputs"][ik], ik);
        let nears = geometries(&reference["nears"][nk], nk);
        for (radius, rows) in [
            (None, &c["rows"]),
            (
                Some(c["radius_km"].as_f64().unwrap() * 1000.0),
                &c["rows_radius"],
            ),
        ] {
            for (n, g) in inputs.iter().enumerate() {
                let mut best: Option<(usize, geoforge_geometry::Nearest)> = None;
                for (i, ng) in nears.iter().enumerate() {
                    let hit = nearest_geodesic(g, ng).unwrap().unwrap();
                    if radius.is_none_or(|r| hit.distance <= r)
                        && best.is_none_or(|(_, b)| hit.distance < b.distance)
                    {
                        best = Some((i, hit));
                    }
                }
                let want = rows[n].as_array().unwrap();
                let w = |k: usize| want[k].as_f64().unwrap();
                let label = format!("{ik}/{nk} #{n} radius {radius:?}");
                match best {
                    None => {
                        if w(0) != -1.0 {
                            failures.push(format!("{label}: ArcGIS found FID {}", w(0)));
                        }
                    }
                    Some((i, h)) => {
                        let rel = (h.distance - w(1)).abs() / w(1).max(1.0);
                        worst = worst.max(rel);
                        let angle = if h.distance == 0.0 {
                            0.0
                        } else {
                            azimuth(h.from, h.to)
                        };
                        // ArcGIS finds the closest point on a plain line only approximately (up to
                        // 0.08 % off in distance; the distance is flat near the minimum, so the
                        // location and angle move more).
                        let (rel, loc, deg) = if ik == "lines" || nk == "lines" {
                            (1e-3, 0.1, 0.3)
                        } else {
                            (2e-4, 0.01, 0.5)
                        };
                        let ok = w(0) as usize == i + 1
                            && (h.distance - w(1)).abs() <= 0.001 + rel * w(1)
                            && (h.to[0] - w(2)).abs() < loc
                            && (h.to[1] - w(3)).abs() < loc
                            && (angle - w(4)).abs() < deg;
                        if !ok {
                            failures.push(format!("{label}: got FID {} d {:.3} at {:?} angle {angle:.3}; ArcGIS {want:?}", i + 1, h.distance, h.to));
                        }
                    }
                }
            }
        }
    }
    println!("worst relative distance difference {worst:e}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
