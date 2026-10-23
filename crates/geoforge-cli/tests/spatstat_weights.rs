//! Generate Spatial Weights Matrix, Convert Spatial Weights Matrix to Table and the cluster tools
//! with weights files, polygon contiguity and default thresholds, against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/spatstat_weights.json, inputs in spatstat_weights/; probes and the
//! packing script in scripts/research/spatstat/weights/).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::Value;
use geoforge_shapefile::ShapefileReader;
use geoforge_tools::swm::Swm;
use serde_json::Value as Json;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference")
}

fn reference() -> Json {
    serde_json::from_slice(&std::fs::read(root().join("spatstat_weights.json")).unwrap()).unwrap()
}

fn input(name: &str) -> String {
    let dir = root().join("spatstat_weights");
    if name.contains(".gdb") {
        dir.join(name).display().to_string()
    } else {
        dir.join(format!("{name}.shp")).display().to_string()
    }
}

fn gf(args: &[String]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gf-weights-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-300)
}

/// Equidistant neighbors (regular grids, the five-point set) and cocircular Delaunay points:
/// ArcGIS breaks those ties in an order not reproduced (docs/KNOWN_GAPS.md).
const TIES: &[&str] = &[
    "p5knn",
    "g_knn3",
    "g_knn5",
    "g_knn5_man",
    "g_del",
    "g_fix10_k6",
];

/// Rows whose neighbor order follows a Python set (Delaunay, contiguity) are compared as sets.
fn ordered(case: &str) -> bool {
    !(case.contains("del")
        || case.starts_with("bp_")
        || case == "pg_corners_o2"
        || case.starts_with("pg_edges_o"))
}

#[test]
fn weights_files_equal_arcgis() {
    let r = reference();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, c) in r["swm"].as_object().unwrap() {
        if TIES.contains(&case.as_str()) {
            continue;
        }
        checked += 1;
        let out = temp(&format!("{case}.swm"));
        let mut args = vec![
            "run".to_string(),
            "generate_spatial_weights_matrix".to_string(),
            format!("input={}", input(c["input"].as_str().unwrap())),
            format!("unique_id_field={}", c["uid"].as_str().unwrap()),
            format!("output={}", out.display()),
        ];
        for a in c["args"].as_array().unwrap() {
            let a = a.as_str().unwrap();
            args.push(match a.strip_prefix("input_table=") {
                Some(t) => format!(
                    "input_table={}",
                    root().join("spatstat_weights").join(t).display()
                ),
                None => a.to_string(),
            });
        }
        gf(&args);
        let ours = Swm::read(&out).unwrap();
        let header: Vec<String> = ours
            .header
            .iter()
            .filter(|(k, _)| k != "INPUTFC")
            .map(|(k, v)| format!("{k}@{v}"))
            .collect();
        if header.join(";") != c["header"].as_str().unwrap() {
            failures.push(format!("{case}: header {}", header.join(";")));
            continue;
        }
        if ours.row_standardized != (c["rowstd"].as_i64() == Some(1))
            || ours.count as i64 != c["count"].as_i64().unwrap()
        {
            failures.push(format!("{case}: standardization or count"));
            continue;
        }
        let want = c["rows"].as_array().unwrap();
        if want.len() != ours.rows.len() {
            failures.push(format!(
                "{case}: {} rows, ArcGIS {}",
                ours.rows.len(),
                want.len()
            ));
            continue;
        }
        for (o, w) in ours.rows.iter().zip(want) {
            let id = w[0].as_i64().unwrap();
            let nb: Vec<i64> = w[1]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap())
                .collect();
            let mut ws: Vec<f64> = w[2]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            if ws.len() == 1 {
                ws = vec![ws[0]; nb.len()];
            }
            let mut a: Vec<(i64, f64)> = o
                .neighbors
                .iter()
                .copied()
                .zip(o.weights.iter().copied())
                .collect();
            let mut b: Vec<(i64, f64)> = nb.iter().copied().zip(ws.iter().copied()).collect();
            if !ordered(case) {
                a.sort_by_key(|x| x.0);
                b.sort_by_key(|x| x.0);
            }
            let sum_ok = w[3].as_f64().is_none_or(|s| close(o.sum, s, 1e-9));
            let bad = o.id != id
                || a.len() != b.len()
                || a.iter()
                    .zip(&b)
                    .any(|(x, y)| x.0 != y.0 || !close(x.1, y.1, 1e-9))
                || !sum_ok;
            if bad {
                failures.push(format!("{case} row {id}: {:?} but ArcGIS {w}", o));
                break;
            }
        }
        // Convert Spatial Weights Matrix to Table, from our file.
        if let Some(t) = r["tables"].get(case.as_str()) {
            let table = temp(&format!("{case}_table.dbf"));
            gf(&[
                "run".into(),
                "convert_spatial_weights_matrix_to_table".into(),
                format!("input={}", out.display()),
                format!("output={}", table.display()),
            ]);
            let rd = geoforge_shapefile::dbf::DbfReader::open(&table, None).unwrap();
            let names: Vec<String> = rd.header().fields.iter().map(|f| f.name.clone()).collect();
            let want_names: Vec<&str> = t["fields"].as_array().unwrap()[1..]
                .iter()
                .map(|f| f[0].as_str().unwrap())
                .collect();
            if names != want_names {
                failures.push(format!("{case} table: fields {names:?}"));
                continue;
            }
            let rows: Vec<Vec<Value>> = (0..rd.num_records())
                .map(|i| rd.read(i).unwrap().0)
                .collect();
            let want = t["rows"].as_array().unwrap();
            let mut ok = rows.len() == want.len();
            let key = |v: &[Value]| match (&v[1], &v[2]) {
                (Value::Integer(a), Value::Integer(b)) => (*a as i64, *b as i64),
                (Value::BigInteger(a), Value::BigInteger(b)) => (*a, *b),
                _ => (0, 0),
            };
            let mut ours: Vec<(i64, i64, f64)> = rows
                .iter()
                .map(|v| {
                    let (a, b) = key(v);
                    (
                        a,
                        b,
                        if let Value::Double(w) = v[3] {
                            w
                        } else {
                            f64::NAN
                        },
                    )
                })
                .collect();
            let mut theirs: Vec<(i64, i64, f64)> = want
                .iter()
                .map(|w| {
                    (
                        w[2].as_i64().unwrap(),
                        w[3].as_i64().unwrap(),
                        w[4].as_f64().unwrap(),
                    )
                })
                .collect();
            if !ordered(case) {
                ours.sort_by_key(|x| (x.0, x.1));
                theirs.sort_by_key(|x| (x.0, x.1));
            }
            ok &= ours
                .iter()
                .zip(&theirs)
                .all(|(a, b)| a.0 == b.0 && a.1 == b.1 && close(a.2, b.2, 1e-9));
            if !ok {
                failures.push(format!(
                    "{case} table: {:?} but ArcGIS {:?}",
                    &ours[..3.min(ours.len())],
                    &theirs[..3.min(theirs.len())]
                ));
            }
        }
    }
    assert!(checked >= 70, "{checked} weights files checked");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn report(stdout: &str) -> Vec<(String, f64)> {
    stdout
        .lines()
        .filter_map(|l| l.rsplit_once(": "))
        .filter_map(|(k, v)| v.trim().parse().ok().map(|v| (k.to_string(), v)))
        .collect()
}

/// Our weights files for the statistics: generated once per case with gf.
fn weights(case: &str, r: &Json) -> String {
    let out = temp(&format!("stat_{case}.swm"));
    if !out.exists() {
        let c = &r["swm"][case];
        let mut args = vec![
            "run".to_string(),
            "generate_spatial_weights_matrix".to_string(),
            format!("input={}", input(c["input"].as_str().unwrap())),
            format!("unique_id_field={}", c["uid"].as_str().unwrap()),
            format!("output={}", out.display()),
        ];
        args.extend(
            c["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_str().unwrap().to_string()),
        );
        gf(&args);
    }
    out.display().to_string()
}

/// The arguments of a statistics case `<dataset>:<neighborhood>:<standardization>`.
fn stat_args(key: &str, r: &Json) -> Vec<String> {
    let p: Vec<&str> = key.split(':').collect();
    let ds = match p[0] {
        "big" => "big",
        _ => "bpg",
    };
    let mut a = vec![format!("input={}", input(ds)), "field=V".to_string()];
    let n = p[1];
    if n.starts_with("b_") || n.starts_with("bp_") || n.starts_with("swm_") {
        let case = match n {
            "swm_inv" => "b_inv",
            "swm_fix" => "b_fix250",
            "swm_corners" => "bp_corners",
            other => other,
        };
        a.push("conceptualization=GET_SPATIAL_WEIGHTS_FROM_FILE".into());
        a.push(format!("weights_matrix_file={}", weights(case, r)));
    } else {
        let (conc, extra): (&str, &[&str]) = match n {
            "inv" => ("INVERSE_DISTANCE", &[]),
            "invsq" => ("INVERSE_DISTANCE_SQUARED", &[]),
            "fix250" => ("FIXED_DISTANCE_BAND", &["distance_band=250"]),
            "fix" => ("FIXED_DISTANCE_BAND", &[]),
            "zoi" => ("ZONE_OF_INDIFFERENCE", &["distance_band=200"]),
            "knn6" => ("K_NEAREST_NEIGHBORS", &["number_of_neighbors=6"]),
            "edges" => ("CONTIGUITY_EDGES_ONLY", &[]),
            other => (other, &[]),
        };
        a.push(format!("conceptualization={conc}"));
        a.extend(extra.iter().map(|s| s.to_string()));
    }
    if p.get(2) == Some(&"none") {
        a.retain(|x| !x.starts_with("distance_band="));
    }
    match p.get(2).copied() {
        Some("0") => a.push("distance_band=0".into()),
        Some("none") | None => {}
        Some(std) => a.push(format!("standardization={std}")),
    }
    if p.get(3) == Some(&"fdr") {
        a.push("apply_fdr=APPLY_FDR".into());
    }
    a
}

#[test]
fn moran_with_weights_and_contiguity_equals_arcgis() {
    let r = reference();
    let mut failures = Vec::new();
    for (key, want) in r["moran"].as_object().unwrap() {
        let mut a = vec!["run".to_string(), "spatial_autocorrelation".to_string()];
        a.extend(stat_args(key, &r));
        let got = report(&gf(&a));
        let mine = [got[0].1, got[3].1, got[4].1];
        let want: Vec<f64> = want
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        // ArcGIS prints six decimals.
        if mine.iter().zip(&want).any(|(g, w)| (g - w).abs() > 5.1e-7) {
            failures.push(format!("{key}: {mine:?} but ArcGIS {want:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn attrs(path: &Path) -> (Vec<String>, Vec<Vec<Value>>) {
    let rd = ShapefileReader::open(path).unwrap();
    let names = rd.fields().iter().map(|f| f.name.clone()).collect();
    let rows = rd.features().map(|f| f.unwrap().attributes).collect();
    (names, rows)
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Double(v) => *v,
        Value::Integer(v) => *v as f64,
        Value::BigInteger(v) => *v as f64,
        _ => f64::NAN,
    }
}

#[test]
fn hot_spots_and_local_moran_equal_arcgis() {
    let r = reference();
    let mut failures = Vec::new();
    for (tool, table, cols) in [
        // (our column, ArcGIS column) after FID.
        (
            "hot_spot_analysis",
            "hot",
            vec![(0usize, 1usize), (2, 3), (3, 4), (4, 5), (5, 6)],
        ),
        (
            "cluster_and_outlier_analysis",
            "lisa",
            vec![
                (0, 1),
                (2, 3),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 8),
                (8, 9),
            ],
        ),
    ] {
        for (key, want) in r[table].as_object().unwrap() {
            let out = temp(&format!("{tool}_{}.shp", key.replace(':', "_")));
            let mut a = vec![
                "run".to_string(),
                tool.to_string(),
                format!("output={}", out.display()),
            ];
            a.extend(stat_args(key, &r));
            if tool == "cluster_and_outlier_analysis" {
                a.push("number_of_permutations=0".into());
            }
            gf(&a);
            let (names, rows) = attrs(&out);
            let want = want.as_array().unwrap();
            // The two default-band cases keep only some columns.
            let short = key == "big:fix:none" || key == "big:inv:0";
            if !short {
                let fields = if table == "hot" {
                    r["hot_fields"][key.as_str()].clone()
                } else {
                    Json::Array(
                        r["lisa_fields"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|f| f[0].clone())
                            .collect(),
                    )
                };
                let mut want_names: Vec<String> = fields
                    .as_array()
                    .unwrap()
                    .iter()
                    .skip(2)
                    .map(|f| f.as_str().unwrap().to_string())
                    .collect();
                if table == "lisa" && key.contains("swm") {
                    want_names[0] = "UID".into();
                }
                if names != want_names {
                    failures.push(format!("{tool} {key}: fields {names:?} but {want_names:?}"));
                    continue;
                }
            }
            for (i, (g, w)) in rows.iter().zip(want).enumerate() {
                let pairs: Vec<(usize, usize)> = if short {
                    if tool == "hot_spot_analysis" {
                        vec![(2, 0), (4, 1)]
                    } else {
                        vec![(2, 0), (3, 1), (6, 2)]
                    }
                } else {
                    cols.clone()
                };
                let bad = pairs.iter().any(|&(o, t)| match (&g[o], &w[t]) {
                    (Value::String(s), Json::String(t)) => s.trim() != t.trim(),
                    (Value::Null, Json::String(t)) => !t.trim().is_empty(),
                    (v, t) => {
                        let t = t.as_f64().unwrap();
                        // p-values: ArcGIS's normal approximation is a few 1e-8 off.
                        (num(v) - t).abs() > 5e-8 + 1e-9 * t.abs()
                    }
                });
                if bad {
                    failures.push(format!("{tool} {key} row {i}: {g:?} but ArcGIS {w}"));
                    break;
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
