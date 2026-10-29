//! Precision migration changes catalog metadata only; refusals and repeats retain bytes.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_filegdb::{GdbFieldType, Geodatabase};
fn copy(label: &str) -> PathBuf {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/migrate_date_base.gdb");
    let dir = std::env::temp_dir().join(format!(
        "ug-migrate-date-{}-{label}.gdb",
        std::process::id()
    ));
    std::fs::create_dir(&dir).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), dir.join(e.file_name())).unwrap();
        }
    }
    dir
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
fn run(dir: &Path, name: &str, fields: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "migrate_date_field_to_high_precision"])
        .arg(format!("input={}", dir.join(name).display()))
        .arg(format!("date_fields={fields}"))
        .output()
        .unwrap()
}
#[test]
fn catalog_only_and_repeat_and_existing_high_precision_are_byte_identical() {
    let dir = copy("metadata");
    let original = snapshot(&dir);
    let result = run(&dir, "multiple", "D1;D2");
    assert!(result.status.success(), "{result:?}");
    let first = snapshot(&dir);
    for (file, bytes) in &original {
        if !file.starts_with("a00000004.") && !file.starts_with("a00000006.") {
            assert!(
                first.get(file) == Some(bytes),
                "user data/index/header changed: {file}"
            );
        }
    }
    let gdb = Geodatabase::open(&dir).unwrap();
    let table = gdb.open_table("multiple").unwrap();
    assert!(
        table
            .all_fields()
            .iter()
            .filter(|f| f.field_type == GdbFieldType::DateTime)
            .all(|f| f.high_precision)
    );
    drop(table);
    drop(gdb);
    assert!(run(&dir, "multiple", "d1;d2").status.success());
    assert!(snapshot(&dir) == first);
    assert!(run(&dir, "already", "D1").status.success());
    assert!(snapshot(&dir) == first);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_and_mixed_requests_are_preflighted_without_partial_changes() {
    let dir = copy("refusals");
    let before = snapshot(&dir);
    for (name, fields, code) in [
        ("wrong", "KEY", "000640"),
        ("missing", "absent", "000728"),
        ("mixed", "D1;KEY", "000640"),
        ("duplicate", "D1;D1", "000400"),
        ("no_fields", "", "000735"),
    ] {
        let result = run(&dir, name, fields);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(code));
        assert!(snapshot(&dir) == before, "refusal changed bytes");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
