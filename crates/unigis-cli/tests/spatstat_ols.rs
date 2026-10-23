//! Ordinary Least Squares and Spatial Autocorrelation of its residuals against ArcGIS Pro 3.7
//! (testdata/arcgis-reference/spatstat_ols.json; probe scripts/research/spatstat/weights/probe_ols.py).

use std::path::Path;
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::Value;
use unigis_shapefile::ShapefileReader;
use unigis_shapefile::dbf::DbfReader;

fn gf(args: &[String]) -> String {
    let o = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        o.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn num(v: &Value) -> f64 {
    match v {
        Value::Double(v) => *v,
        Value::Integer(v) => *v as f64,
        Value::BigInteger(v) => *v as f64,
        _ => f64::NAN,
    }
}

/// p-values: ArcGIS's come from approximations of the t, F and chi-squared tails (a few 1e-6
/// apart); other values to ArcGIS's 12 significant digits.
fn close(name: &str, got: f64, want: f64) -> bool {
    let p = name.contains("Prob") || name.ends_with("_Pr");
    if p {
        (got - want).abs() <= 2e-6
    } else {
        (got - want).abs() <= 1e-9 * want.abs().max(1.0)
    }
}

#[test]
fn ols_equals_arcgis() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/arcgis-reference");
    let r: Json =
        serde_json::from_slice(&std::fs::read(root.join("spatstat_ols.json")).unwrap()).unwrap();
    let input = root.join("spatstat_weights/ols.shp");
    let tmp = std::env::temp_dir().join(format!("gf-ols-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut failures = Vec::new();
    for case in ["three", "one"] {
        let c = &r[case];
        let (fc, ct, dt) = (
            tmp.join(format!("{case}.shp")),
            tmp.join(format!("{case}_coef.dbf")),
            tmp.join(format!("{case}_diag.dbf")),
        );
        gf(&[
            "run".into(),
            "ordinary_least_squares".into(),
            format!("input={}", input.display()),
            "unique_id_field=UID".into(),
            "dependent_variable=Y".into(),
            format!(
                "explanatory_variables={}",
                c["explanatory"].as_str().unwrap()
            ),
            format!("output={}", fc.display()),
            format!("coefficient_output_table={}", ct.display()),
            format!("diagnostic_output_table={}", dt.display()),
        ]);
        // The two tables (after ArcGIS's OID).
        for (path, fields, rows) in [(&ct, "coef_fields", "coef"), (&dt, "diag_fields", "diag")] {
            let rd = DbfReader::open(path, None).unwrap();
            let names: Vec<String> = rd.header().fields.iter().map(|f| f.name.clone()).collect();
            let want_names: Vec<String> = c[fields].as_array().unwrap()[1..]
                .iter()
                .map(|f| f.as_str().unwrap().to_string())
                .collect();
            if names != want_names {
                failures.push(format!("{case} {rows}: fields {names:?}"));
                continue;
            }
            let want = c[rows].as_array().unwrap();
            assert_eq!(rd.num_records() as usize, want.len(), "{case} {rows}");
            for (i, w) in want.iter().enumerate() {
                let got = rd.read(i as u32).unwrap().0;
                let label = match &got[1] {
                    Value::String(s) => s.clone(),
                    other => format!("{other:?}"),
                };
                for (k, name) in names.iter().enumerate().skip(1) {
                    let ok = match (&got[k], &w[k + 1]) {
                        (Value::String(s), Json::String(t)) => s.trim() == t.trim(),
                        (v, t) => {
                            let col = if rows == "diag" {
                                label.as_str()
                            } else {
                                name.as_str()
                            };
                            close(col, num(v), t.as_f64().unwrap())
                        }
                    };
                    if !ok {
                        failures.push(format!(
                            "{case} {rows} {label} {name}: {:?} but ArcGIS {}",
                            got[k],
                            w[k + 1]
                        ));
                    }
                }
            }
        }
        // The features: ID, dependent, explanatory, Estimated, Residual, StdResid.
        let rd = ShapefileReader::open(&fc).unwrap();
        let names: Vec<String> = rd.fields().iter().map(|f| f.name.clone()).collect();
        let want_names: Vec<String> = c["fc_fields"].as_array().unwrap()[2..]
            .iter()
            .map(|f| f.as_str().unwrap().to_string())
            .collect();
        assert_eq!(names, want_names, "{case} features");
        for (f, w) in rd.features().zip(c["fc"].as_array().unwrap()) {
            let f = f.unwrap();
            for (k, v) in f.attributes.iter().enumerate() {
                let t = w[k + 1].as_f64().unwrap();
                if !close("", num(v), t) {
                    failures.push(format!(
                        "{case} feature {}: {:?} but ArcGIS {w}",
                        f.oid, f.attributes
                    ));
                    break;
                }
            }
        }
        if case == "three" {
            let out = gf(&[
                "run".into(),
                "spatial_autocorrelation".into(),
                format!("input={}", fc.display()),
                "field=StdResid".into(),
                "conceptualization=INVERSE_DISTANCE".into(),
                "standardization=ROW".into(),
            ]);
            let got: Vec<f64> = out
                .lines()
                .filter_map(|l| l.rsplit_once(": ").and_then(|(_, v)| v.trim().parse().ok()))
                .collect();
            let want: Vec<f64> = r["resid_moran"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let mine = [got[0], got[3], got[4]];
            if mine.iter().zip(&want).any(|(g, w)| (g - w).abs() > 5.1e-7) {
                failures.push(format!("residual Moran's I {mine:?} but ArcGIS {want:?}"));
            }
        }
    }
    std::fs::remove_dir_all(&tmp).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
