//! Random partition membership varies; count, order, record identity and complete
//! complementary coverage are checked against full public all-record baselines.
use serde_json::Value as Json;
use std::{collections::BTreeMap, path::Path, process::Command};
use unigis_core::{Feature, FeatureClass, FieldType, Value};
use unigis_shapefile::{DbfTable, ShapefileReader};
fn open(path: &Path, table: bool) -> Box<dyn FeatureClass> {
    if table {
        Box::new(DbfTable::open(path).unwrap())
    } else {
        Box::new(ShapefileReader::open(path).unwrap())
    }
}
fn key(f: &Feature, fields: &[unigis_core::Field]) -> i64 {
    let i = fields.iter().position(|f| f.name == "KEY").unwrap();
    match f.attributes[i] {
        Value::Integer(n) => i64::from(n),
        _ => panic!("invalid fixture KEY"),
    }
}
pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let dir = out_dir.join(case);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir(&dir).unwrap();
    let mut args = vec!["run".into(), "subset_features".into()];
    for (k, v) in info["run"]["params"].as_object().unwrap() {
        let s = v
            .as_str()
            .unwrap()
            .replace("<out>", &dir.display().to_string());
        args.push(format!("{k}={s}"));
    }
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    let messages = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let mut diffs = Vec::new();
    if info["status"] != "succeeded" {
        if result.status.success() {
            diffs.push("expected refusal".into());
        }
        for m in info["messages"].as_array().unwrap() {
            for c in m
                .as_str()
                .unwrap()
                .lines()
                .filter_map(|s| s.strip_prefix("ERROR ").and_then(|s| s.get(..6)))
            {
                if !messages.contains(c) {
                    diffs.push(format!("missing ERROR {c}: {messages}"));
                }
            }
        }
        if std::fs::read_dir(&dir).unwrap().next().is_some() {
            diffs.push("failed request left partial outputs".into());
        }
        return diffs;
    }
    if !result.status.success() {
        return vec![format!("native subset failed: {messages}")];
    }
    let table = info["table_output"] == true;
    let ext = if table { "dbf" } else { "shp" };
    let reference = root.join("testdata/arcgis-reference").join(case);
    let baseline = root
        .join("testdata/arcgis-reference")
        .join(if table {
            "subf_records_all"
        } else {
            "subf_all"
        })
        .join(format!("training.{ext}"));
    let all = open(&baseline, table);
    let fields = all.fields().to_vec();
    let source = all.features().collect::<Result<Vec<_>, _>>().unwrap();
    let source_keys: Vec<_> = source.iter().map(|f| key(f, &fields)).collect();
    let source_path = root.join(info["run"]["params"]["input"].as_str().unwrap());
    let raw_table = unigis_filegdb::Geodatabase::open(source_path.parent().unwrap())
        .unwrap()
        .open_table(source_path.file_name().unwrap().to_str().unwrap())
        .unwrap();
    let raw_rows: BTreeMap<_, _> = raw_table
        .features()
        .map(|r| {
            let r = r.unwrap();
            (key(&r, raw_table.fields()), r)
        })
        .collect();
    let records: BTreeMap<_, _> = source.iter().map(|f| (key(f, &fields), f)).collect();
    assert_eq!(records.len(), source.len());
    let test = info["run"]["params"]
        .get("out_test_feature_class")
        .is_some();
    let mut union = Vec::new();
    for name in if test {
        vec!["training", "testing"]
    } else {
        vec!["training"]
    } {
        let a = open(&dir.join(format!("{name}.{ext}")), table);
        let b = open(&reference.join(format!("{name}.{ext}")), table);
        if a.fields() != b.fields() {
            diffs.push(format!("{name} schema differs"));
        }
        if a.geometry_kind() != b.geometry_kind()
            || a.has_z() != b.has_z()
            || a.has_m() != b.has_m()
        {
            diffs.push(format!("{name} geometry type differs"));
        }
        if a.spatial_reference_wkt() != b.spatial_reference_wkt() {
            diffs.push(format!("{name} CRS differs"));
        }
        let rows = a.features().collect::<Result<Vec<_>, _>>().unwrap();
        let expected = b.features().count();
        if rows.len() != expected {
            diffs.push(format!("{name} count {} != {expected}", rows.len()));
        }
        let mut order = Vec::new();
        // The expected DBF carry rule is based on public NULL controls and the
        // native partition's row order, independent of its random member choices.
        let mut previous: Vec<Option<Value>> = a
            .fields()
            .iter()
            .map(|f| match f.field_type {
                FieldType::String => Some(Value::String(" ".into())),
                FieldType::SmallInteger => Some(Value::SmallInteger(0)),
                FieldType::Integer => Some(Value::Integer(0)),
                FieldType::Single => Some(Value::Single(0.0)),
                FieldType::Double => Some(Value::Double(0.0)),
                _ => None,
            })
            .collect();
        for row in rows {
            let k = key(&row, a.fields());
            union.push(k);
            let Some(original) = records.get(&k) else {
                diffs.push(format!("unexpected source record {k}"));
                continue;
            };
            order.push(source_keys.iter().position(|i| *i == k).unwrap());
            let mut expected = original.attributes.clone();
            let raw = &raw_rows[&k];
            for (i, prior) in previous.iter_mut().enumerate() {
                if let Some(prior) = prior {
                    if raw.attributes[i].is_null() {
                        expected[i] = prior.clone();
                    } else {
                        *prior = expected[i].clone();
                    }
                }
            }
            if row.attributes != expected {
                diffs.push(format!(
                    "{name} row {k} attributes differ: {:?} != {:?}",
                    row.attributes, expected
                ));
            }
            if let Some(why) = unigis_testkit::compare_geometries_tol(
                row.geometry.as_ref(),
                original.geometry.as_ref(),
                0.001,
            ) {
                diffs.push(format!("{name} row {k}: {why}"));
            }
        }
        if !order.windows(2).all(|w| w[0] < w[1]) {
            diffs.push(format!("{name} input order not retained"));
        }
    }
    let mut sorted = union.clone();
    sorted.sort();
    sorted.dedup();
    if sorted.len() != union.len() {
        diffs.push("duplicate/overlapping partition members".into());
    }
    if test && sorted != source_keys {
        diffs.push("partition not exhaustive".into());
    }
    for m in info["messages"].as_array().unwrap() {
        for c in m
            .as_str()
            .unwrap()
            .lines()
            .filter_map(|s| s.strip_prefix("WARNING ").and_then(|s| s.get(..6)))
        {
            if !messages.contains(c) {
                diffs.push(format!("missing WARNING {c}"));
            }
        }
    }
    diffs
}
