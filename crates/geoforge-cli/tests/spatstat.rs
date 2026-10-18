//! Spatial Statistics (Mean Center, Median Center, Central Feature, Standard Distance, Directional
//! Distribution, Linear Directional Mean) against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/spatstat.json: two point sets and one line set, each tool with weights,
//! case fields and options).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::{Geometry, Value};
use geoforge_shapefile::ShapefileReader;
use serde_json::Value as Json;

fn shapefile(dir: &Path, name: &str, ds: &Json) -> PathBuf {
    let line = ds["kind"] == "line";
    let features: Vec<String> = ds["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            if line {
                format!(
                    r#"{{"type":"Feature","properties":{{"G":"{}"}},"geometry":{{"type":"LineString","coordinates":[[{},{}],[{},{}]]}}}}"#,
                    r[4].as_str().unwrap(),
                    r[0],
                    r[1],
                    r[2],
                    r[3]
                )
            } else {
                format!(
                    r#"{{"type":"Feature","properties":{{"W":{},"G":"{}","Z":{}}},"geometry":{{"type":"Point","coordinates":[{},{}]}}}}"#,
                    r[2],
                    r[3].as_str().unwrap(),
                    r[4],
                    r[0],
                    r[1]
                )
            }
        })
        .collect();
    let json = dir.join(format!("{name}.geojson"));
    std::fs::write(
        &json,
        format!(
            r#"{{"type":"FeatureCollection","features":[{}]}}"#,
            features.join(",")
        ),
    )
    .unwrap();
    let shp = dir.join(format!("{name}.shp"));
    let r = Command::new("ogr2ogr")
        .args(["-f", "ESRI Shapefile", "-a_srs", "EPSG:32632"])
        .arg(&shp)
        .arg(&json)
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    shp
}

fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(v) => Some(*v as f64),
        Value::SmallInteger(v) => Some(*v as f64),
        Value::BigInteger(v) => Some(*v as f64),
        Value::Double(v) => Some(*v),
        Value::Single(v) => Some(*v as f64),
        _ => None,
    }
}

/// Our output against an ArcGIS case: attribute values then geometry (vertices).
fn compare(key: &str, out: &Path, want: &Json, tolerance: f64) -> Vec<String> {
    let mut failures = Vec::new();
    let r = ShapefileReader::open(out).unwrap();
    let ours: Vec<_> = r.features().map(|f| f.unwrap()).collect();
    let rows = want["rows"].as_array().unwrap();
    if ours.len() != rows.len() {
        return vec![format!(
            "{key}: {} features but ArcGIS {}",
            ours.len(),
            rows.len()
        )];
    }
    let want_fields: Vec<&str> = want["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f[0].as_str().unwrap())
        .filter(|n| *n != "FID" && *n != "Shape")
        .collect();
    let our_fields: Vec<&str> = r.fields().iter().map(|f| f.name.as_str()).collect();
    if our_fields != want_fields {
        failures.push(format!(
            "{key}: fields {our_fields:?} but ArcGIS {want_fields:?}"
        ));
        return failures;
    }
    for (i, (f, w)) in ours.iter().zip(rows).enumerate() {
        // attrs: FID first, then the fields.
        for (k, name) in want_fields.iter().enumerate() {
            let a = &w["attrs"][k + 1];
            match (a, f.attributes.get(k)) {
                (Json::Number(n), Some(v)) => {
                    let (want_v, got) = (n.as_f64().unwrap(), num(v));
                    if got.is_none_or(|g| (g - want_v).abs() > tolerance + 1e-9 * want_v.abs()) {
                        failures.push(format!("{key} row {i} {name}: {got:?} but ArcGIS {want_v}"));
                    }
                }
                (Json::String(s), Some(Value::String(g))) => {
                    if s != g.trim_end() {
                        failures.push(format!("{key} row {i} {name}: {g:?} but ArcGIS {s:?}"));
                    }
                }
                (a, v) => failures.push(format!("{key} row {i} {name}: {v:?} but ArcGIS {a}")),
            }
        }
        let g: &Geometry = f.geometry.as_ref().unwrap();
        let theirs: Vec<[f64; 2]> = if let Some(rings) = w["geom"]["rings"].as_array() {
            rings
                .iter()
                .flat_map(|r| {
                    r.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
                })
                .collect()
        } else if let Some(paths) = w["geom"]["paths"].as_array() {
            paths
                .iter()
                .flat_map(|r| {
                    r.as_array()
                        .unwrap()
                        .iter()
                        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
                })
                .collect()
        } else {
            vec![[
                w["geom"]["x"].as_f64().unwrap(),
                w["geom"]["y"].as_f64().unwrap(),
            ]]
        };
        if g.xy.len() != theirs.len() {
            failures.push(format!(
                "{key} row {i}: {} vertices but ArcGIS {}",
                g.xy.len(),
                theirs.len()
            ));
            continue;
        }
        let worst =
            g.xy.iter()
                .zip(&theirs)
                .map(|(a, b)| (a[0] - b[0]).abs().max((a[1] - b[1]).abs()))
                .fold(0.0, f64::max);
        if worst > tolerance.max(1e-3) {
            failures.push(format!("{key} row {i}: geometry differs by {worst}"));
        }
    }
    failures
}

fn run(args: &[String]) {
    let out = Command::new(env!("CARGO_BIN_EXE_gf"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn spatial_statistics_equal_arcgis() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Json = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/spatstat.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-spatstat-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let inputs = [
        ("A", shapefile(&d, "fa", &r["datasets"]["fa"])),
        ("B", shapefile(&d, "fb", &r["datasets"]["fb"])),
        ("L", shapefile(&d, "fl", &r["datasets"]["fl"])),
    ];
    let s = |x: &str| x.to_string();
    // (key suffix, command, extra arguments, tolerance in metres)
    let mut cases: Vec<(&str, &str, Vec<String>, f64)> = Vec::new();
    let w = vec![s("--weight-field"), s("W")];
    let wc = vec![s("--weight-field"), s("W"), s("--case-field"), s("G")];
    for p in ["A", "B"] {
        let _ = p;
        cases.push(("mean", "mean-center", vec![], 1e-3));
        cases.push(("mean_w", "mean-center", w.clone(), 1e-3));
        cases.push(("mean_case", "mean-center", wc.clone(), 1e-3));
        cases.push((
            "mean_dim",
            "mean-center",
            [wc.clone(), vec![s("--dimension-field"), s("Z")]].concat(),
            1e-3,
        ));
        // ArcGIS stops the median iteration by a rule not reproduced: about ten metres at most.
        cases.push(("median", "median-center", vec![], 10.0));
        cases.push(("median_w", "median-center", w.clone(), 10.0));
        cases.push(("median_case", "median-center", wc.clone(), 10.0));
        cases.push(("central", "central-feature", vec![], 1e-3));
        cases.push((
            "central_man_w",
            "central-feature",
            [
                w.clone(),
                vec![s("--distance-method"), s("MANHATTAN_DISTANCE")],
            ]
            .concat(),
            1e-3,
        ));
        cases.push((
            "central_self",
            "central-feature",
            [w.clone(), vec![s("--self-potential-weight-field"), s("Z")]].concat(),
            1e-3,
        ));
        cases.push(("central_case", "central-feature", wc.clone(), 1e-3));
        cases.push(("sd1", "standard-distance", vec![], 1e-3));
        cases.push((
            "sd2_w",
            "standard-distance",
            [
                w.clone(),
                vec![s("--circle-size"), s("2_STANDARD_DEVIATIONS")],
            ]
            .concat(),
            1e-3,
        ));
        cases.push((
            "sd3_case",
            "standard-distance",
            [
                wc.clone(),
                vec![s("--circle-size"), s("3_STANDARD_DEVIATIONS")],
            ]
            .concat(),
            1e-3,
        ));
        cases.push(("ell1", "directional-distribution", vec![], 1e-3));
        cases.push((
            "ell2_w",
            "directional-distribution",
            [
                w.clone(),
                vec![s("--ellipse-size"), s("2_STANDARD_DEVIATIONS")],
            ]
            .concat(),
            1e-3,
        ));
        cases.push((
            "ell3_case",
            "directional-distribution",
            [
                wc.clone(),
                vec![s("--ellipse-size"), s("3_STANDARD_DEVIATIONS")],
            ]
            .concat(),
            1e-3,
        ));
    }
    let mut failures = Vec::new();
    let mut n = 0;
    for (set, shp) in &inputs {
        let applicable = &cases;
        let mut jobs: Vec<(String, &str, Vec<String>, f64)> = Vec::new();
        if *set == "L" {
            jobs.push(("L:ldm".into(), "directional-mean", vec![], 1e-3));
            jobs.push((
                "L:ldm_o".into(),
                "directional-mean",
                vec![s("--orientation-only")],
                1e-3,
            ));
            jobs.push((
                "L:ldm_case".into(),
                "directional-mean",
                vec![s("--case-field"), s("G")],
                1e-3,
            ));
            jobs.push((
                "L:ldm_o_case".into(),
                "directional-mean",
                vec![s("--orientation-only"), s("--case-field"), s("G")],
                1e-3,
            ));
        } else {
            for (suffix, cmd, args, tol) in applicable {
                jobs.push((format!("{set}:{suffix}"), cmd, args.clone(), *tol));
            }
        }
        for (key, cmd, args, tol) in jobs {
            n += 1;
            let out = d.join(format!("o{n}.shp"));
            let mut a = vec![
                s(cmd),
                shp.to_str().unwrap().to_string(),
                out.to_str().unwrap().to_string(),
            ];
            a.extend(args);
            run(&a);
            failures.extend(compare(&key, &out, &r["cases"][&key], tol));
        }
    }
    std::fs::remove_dir_all(&d).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn run_command_uses_the_registry() {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let r: Json = serde_json::from_slice(
        &std::fs::read(root.join("testdata/arcgis-reference/spatstat.json")).unwrap(),
    )
    .unwrap();
    let d = std::env::temp_dir().join(format!("gf-spatstat-run-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let shp = shapefile(&d, "fa", &r["datasets"]["fa"]);
    let out = d.join("o.shp");
    // The same run as the typed command: weights and a case field.
    run(&[
        "run".into(),
        "mean_center".into(),
        format!("input={}", shp.display()),
        format!("output={}", out.display()),
        "weight_field=W".into(),
        "case_field=G".into(),
    ]);
    let failures = compare("A:mean_case", &out, &r["cases"]["A:mean_case"], 1e-3);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // Choices are checked, unknown parameters and tools refused, `tools` lists them.
    for bad in [
        vec![
            "run".to_string(),
            "standard_distance".into(),
            format!("input={}", shp.display()),
            format!("output={}", d.join("x.shp").display()),
            "circle_size=HUGE".into(),
        ],
        vec![
            "run".to_string(),
            "mean_center".into(),
            format!("input={}", shp.display()),
            format!("output={}", d.join("x.shp").display()),
            "nonsense=1".into(),
        ],
        vec!["run".to_string(), "no_such_tool".into(), "output=x".into()],
        vec![
            "run".to_string(),
            "mean_center".into(),
            "input=missing.shp".into(),
        ],
    ] {
        let r = Command::new(env!("CARGO_BIN_EXE_gf"))
            .args(&bad)
            .output()
            .unwrap();
        assert!(!r.status.success(), "{bad:?}");
    }
    let list = Command::new(env!("CARGO_BIN_EXE_gf"))
        .arg("tools")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&list.stdout).contains("directional_distribution"));
    std::fs::remove_dir_all(&d).ok();
}
