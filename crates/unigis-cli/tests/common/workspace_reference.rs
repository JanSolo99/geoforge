//! Registry tools returning a complete persistent workspace, with a saved seed.
use serde_json::Value as Json;
use std::{path::Path, process::Command};
use unigis_core::{FeatureClass, Value};
use unigis_filegdb::Geodatabase;
use unigis_testkit::compare_geometries_tol;
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir(target).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if e.file_name().to_string_lossy().ends_with(".lock") {
            continue;
        }
        std::fs::copy(e.path(), target.join(e.file_name())).unwrap();
    }
}
fn equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Double(a), Value::Double(b)) => {
            (*a - *b).abs() <= 1e-10 * a.abs().max(b.abs()).max(1.0)
        }
        _ => a == b,
    }
}
pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let out = out_dir.join(format!("{case}.gdb"));
    if out.exists() {
        std::fs::remove_dir_all(&out).unwrap();
    }
    if let Some(seed) = info["workspace_reference"]["seed"].as_str() {
        copy(&root.join(seed), &out);
    }
    let mut args = vec![
        "run".to_owned(),
        info["run"]["tool"].as_str().unwrap().to_owned(),
    ];
    for (k, v) in info["run"]["params"].as_object().unwrap() {
        args.push(format!(
            "{k}={}",
            v.as_str()
                .unwrap()
                .replace("<out>", &out.display().to_string())
        ));
    }
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&result.stderr);
    if info["status"] != "succeeded" {
        let mut diffs = vec![];
        if result.status.code() != Some(1) {
            diffs.push(format!("expected handled failure: {result:?}"));
        }
        for message in info["messages"].as_array().unwrap() {
            for code in message
                .as_str()
                .unwrap()
                .lines()
                .filter_map(|s| s.strip_prefix("ERROR ").and_then(|s| s.get(..6)))
            {
                if !stderr.contains(code) {
                    diffs.push(format!("missing ERROR {code}: {stderr}"));
                }
            }
        }
        return diffs;
    }
    if !result.status.success() {
        return vec![format!("native workspace tool failed: {stderr}")];
    }
    let a = Geodatabase::open(root.join(format!("testdata/arcgis-reference/{case}.gdb"))).unwrap();
    let b = Geodatabase::open(&out).unwrap();
    let mut diffs = vec![];
    let names = |db: &Geodatabase| {
        let mut names: Vec<_> = db
            .entries()
            .iter()
            .map(|e| (e.name.clone(), e.feature_dataset.clone()))
            .collect();
        names.sort();
        names
    };
    if names(&a) != names(&b) {
        diffs.push(format!("catalog {:?} != {:?}", names(&b), names(&a)));
    }
    if a.domains() != b.domains() || a.relationship_classes() != b.relationship_classes() {
        diffs.push("domains/relationships differ".into());
    }
    for entry in a.entries() {
        let x = a.open_table(&entry.name).unwrap();
        let Ok(y) = b.open_table(&entry.name) else {
            continue;
        };
        if x.fields() != y.fields() {
            diffs.push(format!(
                "{} schema {:?} != {:?}",
                entry.name,
                y.fields(),
                x.fields()
            ));
        }
        if x.shape_field_name() != y.shape_field_name()
            || x.geometry_kind() != y.geometry_kind()
            || x.has_z() != y.has_z()
            || x.has_m() != y.has_m()
        {
            diffs.push(format!("{} geometry schema differs", entry.name));
        }
        // ArcGIS's binary WKT stores the degree factor at full f64 precision;
        // a source PRJ can round it. Compare canonical CRS plus exact XY storage.
        let canonical =
            |wkt: Option<&str>| wkt.map(|w| unigis_proj::Crs::from_wkt(w).unwrap().arcgis_wkt());
        if canonical(x.spatial_reference_wkt()) != canonical(y.spatial_reference_wkt()) {
            diffs.push(format!("{} CRS differs", entry.name));
        }
        if let (Some(a), Some(b)) = (x.geometry_def(), y.geometry_def())
            && (a.x_origin, a.y_origin, a.xy_scale) != (b.x_origin, b.y_origin, b.xy_scale)
        {
            diffs.push(format!("{} XY storage differs", entry.name));
        }
        let rowsx = x.features().collect::<Result<Vec<_>, _>>().unwrap();
        let rowsy = y.features().collect::<Result<Vec<_>, _>>().unwrap();
        if rowsx.len() != rowsy.len() {
            diffs.push(format!("{} count differs", entry.name));
        }
        for (x, y) in rowsx.iter().zip(&rowsy) {
            if x.oid != y.oid
                || x.attributes.len() != y.attributes.len()
                || !x
                    .attributes
                    .iter()
                    .zip(&y.attributes)
                    .all(|(a, b)| equal(a, b))
            {
                diffs.push(format!("{} row {y:?} != {x:?}", entry.name));
            }
            if let Some(why) =
                compare_geometries_tol(x.geometry.as_ref(), y.geometry.as_ref(), 0.001)
            {
                diffs.push(format!("{} {why}", entry.name));
            }
        }
    }
    diffs
}
