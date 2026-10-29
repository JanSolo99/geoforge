//! Compare FileGDB schema/values with public ArcGIS controls; refusals preserve data.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_core::{FieldType, Value};
use unigis_filegdb::Geodatabase;
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn temp(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ug-excel-{}-{label}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    dir
}
fn run(ext: &str, out: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "excel_to_table"])
        .arg(format!(
            "input={}",
            root()
                .join(format!("testdata/geoproc/excel_input.{ext}"))
                .display()
        ))
        .arg(format!("output={}", out.display()))
        .args(args)
        .output()
        .unwrap()
}
fn value(v: &Value) -> serde_json::Value {
    use serde_json::json;
    match v {
        Value::Null => serde_json::Value::Null,
        Value::SmallInteger(n) => json!(n),
        Value::Integer(n) => json!(n),
        Value::BigInteger(n) => json!(n),
        Value::Double(n) => json!(n),
        Value::String(s) => json!(s),
        _ => panic!("unexpected Excel value {v:?}"),
    }
}
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn filegdb_schema_aliases_nulls_and_values_match_arcgis() {
    let dir = temp("gdb");
    let oracle: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("scripts/research/excel_to_table/results.json"))
            .unwrap(),
    )
    .unwrap();
    let gdb = dir.join("output.gdb");
    for (name, ext, args) in [
        ("default", "xlsx", vec![]),
        (
            "no_header",
            "xlsx",
            vec!["sheet=Data", "field_names_row=0", "cell_range=A1:C3"],
        ),
        ("range", "xlsx", vec!["sheet=Data", "cell_range=B2:D3"]),
        ("names", "xlsx", vec!["sheet=Names"]),
        ("offset", "xlsx", vec!["sheet=Offset", "field_names_row=3"]),
        ("named", "xlsx", vec!["sheet=Picked"]),
        ("xls", "xls", vec![]),
        ("beyond", "xlsx", vec!["cell_range=A3:C7"]),
    ] {
        let result = run(ext, &gdb.join(name), &args);
        assert!(result.status.success(), "{name}: {result:?}");
        let db = Geodatabase::open(&gdb).unwrap();
        let table = db.open_table(name).unwrap();
        let expected = &oracle[format!("{name}_gdb")];
        let expected_fields = expected["fields"].as_array().unwrap();
        assert_eq!(table.fields().len(), expected_fields.len() - 1, "{name}");
        for (f, e) in table.fields().iter().zip(&expected_fields[1..]) {
            assert_eq!(f.name, e["name"].as_str().unwrap(), "{name}");
            assert_eq!(
                f.alias.as_deref().unwrap_or(&f.name),
                e["alias"].as_str().unwrap(),
                "{name}"
            );
            let kind = match f.field_type {
                FieldType::Integer => "Integer",
                FieldType::BigInteger => "BigInteger",
                FieldType::SmallInteger => "SmallInteger",
                FieldType::Double => "Double",
                FieldType::String => "String",
                _ => panic!("{f:?}"),
            };
            assert_eq!(kind, e["type"].as_str().unwrap(), "{name}/{}", f.name);
            if f.field_type == FieldType::String {
                assert_eq!(f.length, e["length"].as_u64().unwrap() as u32);
            }
        }
        let rows: Vec<Vec<_>> = table
            .features()
            .map(|f| f.unwrap().attributes.iter().map(value).collect())
            .collect();
        let expected_rows = expected["rows"].as_array().unwrap();
        assert_eq!(rows.len(), expected_rows.len(), "{name}");
        for (row, expected) in rows.iter().zip(expected_rows) {
            for (v, e) in row.iter().zip(expected.as_array().unwrap()) {
                if v.is_number() && e.is_number() {
                    assert_eq!(v.as_f64(), e.as_f64(), "{name}");
                } else {
                    assert_eq!(v, e, "{name}");
                }
            }
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_and_unsupported_requests_preserve_outputs_and_workbook() {
    let dir = temp("refusals");
    let out = dir.join("existing.dbf");
    std::fs::write(&out, b"existing output bytes").unwrap();
    std::fs::write(out.with_extension("cpg"), b"existing encoding").unwrap();
    let before = snapshot(&dir);
    let source = root().join("testdata/geoproc/excel_input.xlsx");
    let source_before = std::fs::read(&source).unwrap();
    for args in [
        vec!["sheet=Missing"],
        vec!["sheet=Empty"],
        vec!["cell_range=D3:B2"],
        vec!["cell_range=A0:C2"],
        vec!["field_names_row=-1"],
        vec!["field_names_row=1.5"],
        vec!["field_names_row=8"],
    ] {
        let result = run("xlsx", &out, &args);
        assert_eq!(result.status.code(), Some(1), "{args:?}: {result:?}");
        assert_eq!(snapshot(&dir), before, "{args:?}");
    }
    let missing = dir.join("unwritten.gdb");
    let result = run("xlsx", &missing.join("dates"), &["sheet=Dates"]);
    assert!(!result.status.success(), "{result:?}");
    assert!(!missing.exists());
    assert_eq!(std::fs::read(source).unwrap(), source_before);
    std::fs::remove_dir_all(dir).unwrap();
}
