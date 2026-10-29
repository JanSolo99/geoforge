//! Whole-workspace extraction: catalog, schemas, relationships, domains and rows.
use serde_json::Value as Json;
use std::path::Path;
use std::process::Command;
use unigis_filegdb::Geodatabase;
use unigis_testkit::compare_geometries_tol;

pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let out = out_dir.join(format!("{case}.gdb"));
    if out.exists() {
        std::fs::remove_dir_all(&out).unwrap();
    }
    let existing = info["run"]["params"]["out_type"] == "GEODATABASE";
    if existing {
        unigis_filegdb::GeodatabaseWriter::create(&out)
            .unwrap()
            .finish()
            .unwrap();
    }
    let mut args = vec!["run".into(), "extract_data_from_geodatabase".into()];
    for (k, v) in info["run"]["params"].as_object().unwrap() {
        let value = v
            .as_str()
            .unwrap()
            .replace("<out>", &out.display().to_string())
            .replace("<folder>", &out_dir.display().to_string())
            .replace("<name>", &format!("{case}.gdb"));
        args.push(format!("{k}={value}"));
    }
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(&args)
        .current_dir(root)
        .output()
        .unwrap();
    let mut diffs = Vec::new();
    let stderr = String::from_utf8_lossy(&result.stderr);
    if info["status"] != "succeeded" {
        if result.status.success() {
            diffs.push("expected a refusal".into());
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
        if !existing && out.exists() {
            diffs.push("failed extraction left an output geodatabase".into());
        }
        return diffs;
    }
    if !result.status.success() {
        return vec![format!("native extraction failed: {stderr}")];
    }
    let expected =
        Geodatabase::open(root.join(format!("testdata/arcgis-reference/{case}.gdb"))).unwrap();
    let actual = Geodatabase::open(&out).unwrap();
    let mut want: Vec<_> = expected.entries().iter().map(|e| &e.name).collect();
    want.sort();
    let mut got: Vec<_> = actual.entries().iter().map(|e| &e.name).collect();
    got.sort();
    if want != got {
        diffs.push(format!("catalog: {got:?} != {want:?}"));
    }
    if expected.domains() != actual.domains() {
        diffs.push("domains differ".into());
    }
    if expected.relationship_classes() != actual.relationship_classes() {
        diffs.push("relationships differ".into());
    }
    for name in want {
        let a = expected.open_table(name).unwrap();
        let Ok(b) = actual.open_table(name) else {
            continue;
        };
        if a.fields() != b.fields() {
            diffs.push(format!(
                "{name} fields: {:?} != {:?}",
                b.fields(),
                a.fields()
            ));
        }
        if a.oid_field_name() != b.oid_field_name()
            || a.geometry_kind() != b.geometry_kind()
            || a.has_z() != b.has_z()
            || a.has_m() != b.has_m()
        {
            diffs.push(format!("{name} geometry/OID schema differs"));
        }
        if a.subtypes() != b.subtypes() {
            diffs.push(format!("{name} subtypes differ"));
        }
        let precision = |t: &unigis_filegdb::Table| {
            t.all_fields()
                .iter()
                .map(|f| (f.name.to_lowercase(), f.high_precision))
                .collect::<Vec<_>>()
        };
        if precision(&a) != precision(&b) {
            diffs.push(format!("{name} date precision differs"));
        }
        if let (Some(x), Some(y)) = (a.geometry_def(), b.geometry_def())
            && (x.srs_wkt != y.srs_wkt
                || x.xy_scale != y.xy_scale
                || x.x_origin != y.x_origin
                || x.y_origin != y.y_origin)
        {
            diffs.push(format!("{name} CRS/resolution differs"));
        }
        let indexes = |t: &unigis_filegdb::Table| {
            t.indexes()
                .iter()
                .map(|i| i.expression.to_lowercase())
                .collect::<std::collections::BTreeSet<_>>()
        };
        // ArcGIS assigns destination DSIDs to index names; compare indexed fields.
        if indexes(&a) != indexes(&b) {
            diffs.push(format!(
                "{name} index expressions differ: {:?} != {:?}",
                indexes(&b),
                indexes(&a)
            ));
        }
        let rows_a = a.features().collect::<Result<Vec<_>, _>>().unwrap();
        let rows_b = b.features().collect::<Result<Vec<_>, _>>().unwrap();
        if rows_a.len() != rows_b.len() {
            diffs.push(format!(
                "{name} row count {} != {}",
                rows_b.len(),
                rows_a.len()
            ));
        }
        for (x, y) in rows_a.iter().zip(&rows_b) {
            if x.oid != y.oid || x.attributes != y.attributes {
                diffs.push(format!("{name} row {:?} != {:?}", y, x));
            }
            if let Some(why) =
                compare_geometries_tol(x.geometry.as_ref(), y.geometry.as_ref(), 0.001)
            {
                diffs.push(format!("{name}: {why}"));
            }
        }
    }
    diffs
}
