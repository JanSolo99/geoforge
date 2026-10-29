//! Public EnableCOGO controls require retained values/OID gaps and atomic refusals.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_core::Value;
use unigis_filegdb::Geodatabase;
fn copy(label: &str) -> PathBuf {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/enable_cogo_base.gdb");
    let temp =
        std::env::temp_dir().join(format!("ug-enable-cogo-{}-{label}.gdb", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), temp.join(e.file_name())).unwrap();
        }
    }
    temp
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
fn run(dir: &Path, item: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "enable_cogo"])
        .arg(format!("input={}", dir.join(item).display()))
        .output()
        .unwrap()
}
#[test]
fn dimensions_keep_values_and_deleted_oids_and_repeat_is_byte_identical() {
    let dir = copy("values");
    for name in ["plain", "partial"] {
        let gdb = Geodatabase::open(&dir).unwrap();
        let table = gdb.open_table(name).unwrap();
        let original = table.features().collect::<Result<Vec<_>, _>>().unwrap();
        let count = table.fields().len();
        let prefix = format!(
            "{}.",
            gdb.table_base(name)
                .unwrap()
                .file_stem()
                .unwrap()
                .to_string_lossy()
        );
        drop(table);
        drop(gdb);
        let before = snapshot(&dir);
        let result = run(&dir, name);
        assert!(result.status.success(), "{result:?}");
        let first = snapshot(&dir);
        for (file, bytes) in &before {
            if !file.starts_with(&prefix)
                && !file.starts_with("a00000004.")
                && !file.starts_with("a00000006.")
            {
                assert_eq!(
                    first.get(file),
                    Some(bytes),
                    "unrelated file changed: {file}"
                );
            }
        }
        let gdb = Geodatabase::open(&dir).unwrap();
        let table = gdb.open_table(name).unwrap();
        let rows = table.features().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.iter().map(|r| r.oid).collect::<Vec<_>>(), vec![1, 3]);
        for (old, new) in original.iter().zip(&rows) {
            assert_eq!(old.geometry, new.geometry);
            assert_eq!(old.attributes, new.attributes[..count]);
            assert!(new.attributes[count..].iter().all(Value::is_null));
        }
        for name in ["Direction", "Distance", "Radius", "ArcLength", "Radius2"] {
            assert_eq!(table.field_editable(name), Some(true));
        }
        drop(table);
        drop(gdb);
        assert!(run(&dir, name).status.success());
        assert_eq!(snapshot(&dir), first);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_inputs_and_already_enabled_do_not_change_any_bytes() {
    let dir = copy("refusals");
    let original = snapshot(&dir);
    for (name, code) in [
        ("wrong_string", "002045"),
        ("wrong_integer", "002045"),
        ("point", "003917"),
        ("polygon", "003917"),
        ("table_only", "000732"),
    ] {
        let result = run(&dir, name);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(code));
        assert_eq!(snapshot(&dir), original);
    }
    assert!(run(&dir, "enabled").status.success());
    assert_eq!(snapshot(&dir), original);
    std::fs::remove_dir_all(dir).unwrap();
}
