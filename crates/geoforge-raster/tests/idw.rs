//! Idw against ArcGIS Pro 3.7 (testdata/arcgis-reference/idw.json): the output grid and every cell of
//! runs with each search radius, power and environment.

use std::path::{Path, PathBuf};

use geoforge_core::{Geometry, GeometryKind};
use geoforge_raster::env::{Environment, Mask, Snap};
use geoforge_raster::interpolate::{Sample, SearchRadius, idw};
use serde_json::Value;

fn reference() -> Value {
    let root: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    serde_json::from_slice(&std::fs::read(root.join("testdata/arcgis-reference/idw.json")).unwrap())
        .unwrap()
}

fn samples(v: &Value) -> Vec<Sample> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| Sample {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
            z: p[2].as_f64().unwrap(),
        })
        .collect()
}

/// `got` against an ArcGIS case: grid first, then every cell (-9999 is NoData).
fn check(name: &str, got: &geoforge_raster::Raster, want: &Value) -> Vec<String> {
    let mut failures = Vec::new();
    let e = got.extent();
    let want_extent: Vec<f64> = want["extent"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let (cols, rows) = (
        want["cols"].as_u64().unwrap() as usize,
        want["rows"].as_u64().unwrap() as usize,
    );
    if (got.width, got.height) != (cols, rows)
        || [e.0, e.1, e.2, e.3]
            .iter()
            .zip(&want_extent)
            .any(|(a, b)| (a - b).abs() > 1e-6)
    {
        failures.push(format!(
            "{name}: grid {}x{} {:?} but ArcGIS {cols}x{rows} {want_extent:?}",
            got.width, got.height, e
        ));
        return failures;
    }
    let mut bad = 0;
    let mut first = None;
    for (r, row) in want["values"].as_array().unwrap().iter().enumerate() {
        for (c, w) in row.as_array().unwrap().iter().enumerate() {
            let w = w.as_f64().unwrap();
            let g = got.get(c, r);
            let ok = match g {
                None => w < -9000.0,
                Some(g) => w > -9000.0 && (g - w).abs() <= 1e-3 + 1e-5 * w.abs(),
            };
            if !ok {
                bad += 1;
                first.get_or_insert((r, c, w, g));
            }
        }
    }
    if bad > 0 {
        failures.push(format!("{name}: {bad} cells differ, first {first:?}"));
    }
    failures
}

#[test]
fn search_radii_and_powers_equal_arcgis() {
    let r = reference();
    let pts = samples(&r["points"]);
    let cases: [(&str, f64, SearchRadius); 8] = [
        ("default", 2.0, SearchRadius::default()),
        ("power1", 1.0, SearchRadius::default()),
        ("power3", 3.0, SearchRadius::default()),
        (
            "var5",
            2.0,
            SearchRadius::Variable {
                points: 5,
                max_distance: None,
            },
        ),
        (
            "var5_max",
            2.0,
            SearchRadius::Variable {
                points: 5,
                max_distance: Some(150.0),
            },
        ),
        (
            "fixed",
            2.0,
            SearchRadius::Fixed {
                distance: Some(150.0),
                min_points: 1,
            },
        ),
        (
            "fixed_min3",
            2.0,
            SearchRadius::Fixed {
                distance: Some(150.0),
                min_points: 3,
            },
        ),
        (
            "fixed_def",
            2.0,
            SearchRadius::Fixed {
                distance: None,
                min_points: 0,
            },
        ),
    ];
    let env = Environment {
        cell_size: Some(20.0),
        ..Default::default()
    };
    let mut failures = Vec::new();
    for (name, power, search) in cases {
        let out = idw(&pts, power, search, &env, Some(32632)).unwrap();
        failures.extend(check(name, &out, &r["param_cases"][name]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn default_grids_equal_arcgis() {
    let r = reference();
    let mut failures = Vec::new();
    for g in r["grid"].as_array().unwrap() {
        // Only the grids were kept for these runs: the points are a random set of ten.
        let p = &g["pts"];
        let (x0, y0, x1, y1) = (
            p[0].as_f64().unwrap(),
            p[1].as_f64().unwrap(),
            p[2].as_f64().unwrap(),
            p[3].as_f64().unwrap(),
        );
        // Two corner points make the same extent.
        let pts = [
            Sample {
                x: x0,
                y: y0,
                z: 1.0,
            },
            Sample {
                x: x1,
                y: y1,
                z: 2.0,
            },
        ];
        let env = Environment {
            cell_size: Some(g["cell"].as_f64().unwrap()),
            ..Default::default()
        };
        let out = idw(&pts, 2.0, SearchRadius::default(), &env, None).unwrap();
        let e = out.extent();
        let want: Vec<f64> = g["extent"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let (cols, rows) = (
            g["cols"].as_u64().unwrap() as usize,
            g["rows"].as_u64().unwrap() as usize,
        );
        if (out.width, out.height) != (cols, rows)
            || [e.0, e.1, e.2, e.3]
                .iter()
                .zip(&want)
                .any(|(a, b)| (a - b).abs() > 1e-6)
        {
            failures.push(format!(
                "seed {} cell {}: {}x{} {:?} but ArcGIS {cols}x{rows} {want:?}",
                g["seed"], g["cell"], out.width, out.height, e
            ));
        }
    }
    // No cell size: a 250th of the shorter side.
    let d = &r["default_cell"];
    let p = &d["pts"];
    let pts = [
        Sample {
            x: p[0].as_f64().unwrap(),
            y: p[1].as_f64().unwrap(),
            z: 1.0,
        },
        Sample {
            x: p[2].as_f64().unwrap(),
            y: p[3].as_f64().unwrap(),
            z: 2.0,
        },
    ];
    let out = idw(
        &pts,
        2.0,
        SearchRadius::default(),
        &Environment::default(),
        None,
    )
    .unwrap();
    assert_eq!(
        (out.width, out.height),
        (
            d["cols"].as_u64().unwrap() as usize,
            d["rows"].as_u64().unwrap() as usize
        )
    );
    assert!((out.cell_width() - d["cell"].as_f64().unwrap()).abs() < 1e-9);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn environments_equal_arcgis() {
    let r = reference();
    let pts = samples(&r["env_points"]);
    let mask_ring = &r["mask_polygon"];
    let mut polygon = Geometry::point(0.0, 0.0);
    polygon.kind = GeometryKind::Polygon;
    polygon.parts = vec![0];
    polygon.xy = mask_ring
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
        .collect();
    let snap = &r["snap"];
    let cell = snap["cell"].as_f64().unwrap();
    let mut failures = Vec::new();
    for name in ["extent", "snap", "mask", "cell_env"] {
        let mut env = Environment::default();
        let mut cell_arg = Some(20.0);
        match name {
            "extent" => env.extent = Some([500103.3, 5000041.7, 500377.9, 5000290.2]),
            "snap" => {
                env.snap = Some(Snap {
                    x: snap["x"].as_f64().unwrap(),
                    y: snap["y"].as_f64().unwrap(),
                    cell_w: cell,
                    cell_h: cell,
                })
            }
            "mask" => env.mask = Some(Mask::Polygons(vec![polygon.clone()])),
            _ => {
                env.cell_size = Some(15.0);
                cell_arg = None;
            }
        }
        if let Some(c) = cell_arg {
            env.cell_size = Some(c);
        }
        let out = idw(&pts, 2.0, SearchRadius::default(), &env, None).unwrap();
        failures.extend(check(name, &out, &r["env"][name]));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_variable_radius_stops_at_the_grids_width_plus_height() {
    let r = reference();
    let mut failures = Vec::new();
    for c in r["limit"].as_array().unwrap() {
        let (w, h) = (c["W"].as_f64().unwrap(), c["H"].as_f64().unwrap());
        let env = Environment {
            cell_size: Some(10.0),
            extent: Some([500_000.0, 5_000_000.0, 500_000.0 + w, 5_000_000.0 + h]),
            ..Default::default()
        };
        let out = idw(
            &samples(&c["pts"]),
            2.0,
            SearchRadius::default(),
            &env,
            None,
        )
        .unwrap();
        failures.extend(check(&format!("{w}x{h} +{}", c["off"]), &out, c));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
