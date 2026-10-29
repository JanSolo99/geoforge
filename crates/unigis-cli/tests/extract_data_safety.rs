//! Extraction never mutates its source or unrelated destination data; failed writes roll back.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_core::{Feature, Field, FieldType, Value};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};
fn root(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ug-extract-{}-{label}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    dir
}
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/extract_base.gdb")
}
fn bytes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .filter(|(n, _)| !n.ends_with(".lock"))
        .collect()
}
fn sentinel(dir: &Path) {
    let mut w = GeodatabaseWriter::create(dir).unwrap();
    let mut t = w
        .create_table(
            "sentinel",
            None,
            false,
            false,
            &[Field::new("VALUE", FieldType::Integer)],
            None,
        )
        .unwrap();
    t.write(&Feature {
        oid: 0,
        geometry: None,
        attributes: vec![Value::Integer(77)],
    })
    .unwrap();
    w.finish_table(t).unwrap();
    w.finish().unwrap();
}
fn run(source: &Path, destination: &Path, items: &[&str], new: bool) -> std::process::Output {
    let input = items
        .iter()
        .map(|n| format!("'{}'", source.join(n).display()))
        .collect::<Vec<_>>()
        .join(";");
    let mut c = Command::new(env!("CARGO_BIN_EXE_ug"));
    c.args(["run", "extract_data_from_geodatabase"])
        .arg(format!("in_data={input}"));
    if new {
        c.arg("out_type=NEW_FILE_GEODATABASE")
            .arg(format!(
                "out_folder_path={}",
                destination.parent().unwrap().display()
            ))
            .arg(format!(
                "out_name={}",
                destination.file_name().unwrap().to_string_lossy()
            ));
    } else {
        c.arg(format!("out_geodatabase={}", destination.display()));
    }
    c.output().unwrap()
}
#[test]
fn existing_output_keeps_unrelated_data_and_inputs_and_refuses_collisions() {
    let dir = root("existing");
    let out = dir.join("output.gdb");
    sentinel(&out);
    let source = fixture();
    let before_source = bytes(&source);
    let before_out = bytes(&out);
    let result = run(&source, &out, &["sites"], false);
    assert!(result.status.success(), "{result:?}");
    let db = Geodatabase::open(&out).unwrap();
    let base = db.table_base("sentinel").unwrap();
    for (name, data) in &before_out {
        if name.starts_with(base.file_stem().unwrap().to_str().unwrap()) {
            assert_eq!(std::fs::read(out.join(name)).unwrap(), *data);
        }
    }
    assert_eq!(
        db.open_table("sentinel")
            .unwrap()
            .features()
            .next()
            .unwrap()
            .unwrap()
            .attributes,
        vec![Value::Integer(77)]
    );
    let sites = db.open_table("sites").unwrap();
    assert_eq!(
        sites.features().map(|r| r.unwrap().oid).collect::<Vec<_>>(),
        [1, 3, 4]
    );
    assert_eq!(db.open_table("observations").unwrap().row_count(), 3);
    let after = bytes(&out);
    assert!(!run(&source, &out, &["sites"], false).status.success());
    assert_eq!(bytes(&out), after);
    assert!(
        !run(&source, &out, &["plain", "absent"], false)
            .status
            .success()
    );
    assert_eq!(bytes(&out), after);
    assert_eq!(bytes(&source), before_source);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn mid_write_failure_rolls_back_existing_and_removes_new_output() {
    let dir = root("rollback");
    let source = dir.join("source.gdb");
    std::fs::create_dir(&source).unwrap();
    for e in std::fs::read_dir(fixture()).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), source.join(e.file_name())).unwrap();
        }
    }
    let db = Geodatabase::open(&source).unwrap();
    let ix = db.table_base("sites").unwrap().with_extension("gdbindexes");
    let mut data = std::fs::read(&ix).unwrap();
    // A syntactically readable but unsupported index expression forces failure after
    // the alphabetically earlier plain table has been written into the transaction.
    let key = [b'K', 0, b'E', 0, b'Y', 0];
    let matches = data
        .windows(key.len())
        .enumerate()
        .filter_map(|(i, w)| (w == key).then_some(i))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1);
    data[matches[0]] = b'X';
    std::fs::write(ix, data).unwrap();
    let before_source = bytes(&source);
    let out = dir.join("existing.gdb");
    sentinel(&out);
    let before = bytes(&out);
    let result = run(&source, &out, &["plain", "sites"], false);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("unsupported index expression"),
        "{result:?}"
    );
    assert_eq!(bytes(&out), before);
    assert_eq!(bytes(&source), before_source);
    let new = dir.join("new.gdb");
    assert!(
        !run(&source, &new, &["plain", "sites"], true)
            .status
            .success()
    );
    assert!(!new.exists());
    assert_eq!(bytes(&source), before_source);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn schema_only_retains_high_precision_even_without_rows() {
    let dir = root("precision");
    let out = dir.join("output.gdb");
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/migrate_date_base.gdb");
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args([
            "run",
            "extract_data_from_geodatabase",
            "extract_type=SCHEMA_ONLY",
            "out_type=NEW_FILE_GEODATABASE",
        ])
        .arg(format!("in_data={}", source.join("already").display()))
        .arg(format!("out_folder_path={}", dir.display()))
        .arg("out_name=output")
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let a = Geodatabase::open(source)
        .unwrap()
        .open_table("already")
        .unwrap();
    let b = Geodatabase::open(out)
        .unwrap()
        .open_table("already")
        .unwrap();
    assert!(a.all_fields().iter().any(|f| f.high_precision));
    assert_eq!(b.row_count(), 0);
    assert_eq!(
        a.all_fields()
            .iter()
            .map(|f| f.high_precision)
            .collect::<Vec<_>>(),
        b.all_fields()
            .iter()
            .map(|f| f.high_precision)
            .collect::<Vec<_>>()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn schema_expansion_default_differs_from_explicit_related_request() {
    let dir = root("dynamic-default");
    for (name, explicit, expected_rows) in [("omitted", false, 0), ("explicit", true, 3)] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ug"));
        cmd.args([
            "run",
            "extract_data_from_geodatabase",
            "out_type=NEW_FILE_GEODATABASE",
            "expand_feature_classes_and_tables=ADD_WITH_SCHEMA_ONLY",
        ])
        .arg(format!("in_data={}", fixture().join("sites").display()))
        .arg(format!("out_folder_path={}", dir.display()))
        .arg(format!("out_name={name}"));
        if explicit {
            cmd.arg("get_related_data=GET_RELATED");
        }
        let result = cmd.output().unwrap();
        assert!(result.status.success(), "{result:?}");
        let db = Geodatabase::open(dir.join(format!("{name}.gdb"))).unwrap();
        assert_eq!(
            db.open_table("observations").unwrap().row_count(),
            expected_rows
        );
        assert_eq!(db.open_table("sites").unwrap().row_count(), 3);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
