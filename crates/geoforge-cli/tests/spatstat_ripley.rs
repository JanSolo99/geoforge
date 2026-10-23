//! Multi-Distance Spatial Cluster Analysis (Ripley's k-function) against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/spatstat_ripley.json; inputs in spatstat_weights/, probe in
//! scripts/research/spatstat/weights/probe_ripley.py). With Ripley's edge correction only the
//! bands before circles reach a corner of the study area are compared (docs/KNOWN_GAPS.md).

use std::path::Path;
use std::process::Command;

use geoforge_core::Value;
use serde_json::Value as Json;

#[test]
fn k_function_equals_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let r: Json =
        serde_json::from_slice(&std::fs::read(root.join("spatstat_ripley.json")).unwrap()).unwrap();
    let dir = root.join("spatstat_weights");
    let tmp = std::env::temp_dir().join(format!("gf-ripley-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (case, c) in r.as_object().unwrap() {
        let Some(args) = c.get("args") else { continue };
        checked += 1;
        let out = tmp.join(format!("{case}.dbf"));
        let mut a = vec![
            "run".to_string(),
            "multi_distance_spatial_clustering".to_string(),
            format!(
                "input={}",
                dir.join(format!("{}.shp", c["input"].as_str().unwrap()))
                    .display()
            ),
            format!("output={}", out.display()),
        ];
        for x in args.as_array().unwrap() {
            let x = x.as_str().unwrap();
            a.push(match x.strip_prefix("study_area_feature_class=") {
                Some(f) => format!(
                    "study_area_feature_class={}",
                    dir.join(format!("{f}.shp")).display()
                ),
                None => x.to_string(),
            });
        }
        let o = Command::new(env!("CARGO_BIN_EXE_gf"))
            .args(&a)
            .output()
            .unwrap();
        assert!(
            o.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&o.stderr)
        );
        let rd = geoforge_shapefile::dbf::DbfReader::open(&out, None).unwrap();
        let names: Vec<String> = rd.header().fields.iter().map(|f| f.name.clone()).collect();
        let want_names: Vec<&str> = c["fields"].as_array().unwrap()[1..]
            .iter()
            .map(|f| f.as_str().unwrap())
            .collect();
        if names != want_names {
            failures.push(format!("{case}: fields {names:?}"));
            continue;
        }
        let want = c["rows"].as_array().unwrap();
        let exact = c["exact_rows"].as_u64().unwrap() as usize;
        if rd.num_records() as usize != want.len() {
            failures.push(format!("{case}: {} rows", rd.num_records()));
            continue;
        }
        for (i, w) in want.iter().enumerate().take(exact) {
            let row = rd.read(i as u32).unwrap().0;
            for k in 0..3 {
                let g = match row[k + 1] {
                    Value::Double(v) => v,
                    _ => f64::NAN,
                };
                let t = w[k].as_f64().unwrap();
                // ArcGIS's dBASE values carry 12 significant digits.
                if (g - t).abs() > 1e-9 * t.abs().max(1.0) {
                    failures.push(format!("{case} row {i}: {row:?} but ArcGIS {w}"));
                }
            }
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    assert!(checked >= 9);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
