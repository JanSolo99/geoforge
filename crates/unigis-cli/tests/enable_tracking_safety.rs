//! Enabling tracking preserves existing data and refuses invalid schemas atomically.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_filegdb::Geodatabase;
fn copy(label: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/geoproc/enable_tracking_base.gdb");
    let dir = std::env::temp_dir().join(format!(
        "ug-enable-tracking-{}-{label}.gdb",
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
fn run(dir: &Path, name: &str, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "enable_editor_tracking"])
        .arg(format!("input={}", dir.join(name).display()))
        .args(args)
        .output()
        .unwrap()
}
const ALL: &[&str] = &[
    "creator_field=maker",
    "creation_date_field=made",
    "last_editor_field=changer",
    "last_edit_date_field=changed",
];
#[test]
fn fields_added_without_backfilling_and_existing_values_preserved() {
    let dir = copy("values");
    for name in ["add_all", "table_add", "existing"] {
        let gdb = Geodatabase::open(&dir).unwrap();
        let t = gdb.open_table(name).unwrap();
        let old = t.features().collect::<Result<Vec<_>, _>>().unwrap();
        let fields = t.fields().len();
        let stem = gdb
            .table_base(name)
            .unwrap()
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        drop(t);
        drop(gdb);
        let before = snapshot(&dir);
        let mut args = ALL.to_vec();
        args.push("add_fields=ADD_FIELDS");
        let result = run(&dir, name, &args);
        assert!(result.status.success(), "{result:?}");
        let after = snapshot(&dir);
        for (file, bytes) in &before {
            if !file.starts_with(&format!("{stem}."))
                && !file.starts_with("a00000004.")
                && !file.starts_with("a00000006.")
            {
                assert!(
                    after.get(file) == Some(bytes),
                    "unrelated file changed: {file}"
                );
            }
        }
        let gdb = Geodatabase::open(&dir).unwrap();
        let t = gdb.open_table(name).unwrap();
        let new = t.features().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(old.len(), new.len());
        for (a, b) in old.iter().zip(new) {
            assert_eq!(a.oid, b.oid);
            assert_eq!(a.geometry, b.geometry);
            assert_eq!(a.attributes, b.attributes[..fields]);
            assert!(b.attributes[fields..].iter().all(|v| v.is_null()));
        }
        for f in ["maker", "made", "changer", "changed"] {
            assert_eq!(t.field_editable(f), Some(false));
        }
        drop(t);
        drop(gdb);
        assert!(run(&dir, name, &args).status.success());
        assert!(snapshot(&dir) == after, "repeat changed bytes");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_inputs_and_active_role_changes_preserve_all_bytes() {
    let dir = copy("refusals");
    let original = snapshot(&dir);
    for (name, args, code) in [
        ("no_roles", vec![], "001218"),
        ("missing", ALL.to_vec(), "001278"),
        (
            "wrong",
            vec!["creator_field=maker", "add_fields=true"],
            "001217",
        ),
        (
            "shared",
            vec!["creator_field=maker", "last_editor_field=maker"],
            "001214",
        ),
    ] {
        let result = run(&dir, name, &args);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(code));
        assert!(snapshot(&dir) == original, "refusal changed bytes");
    }
    for (name, args) in [
        (
            "active_same",
            vec!["creator_field=maker", "creation_date_field=made"],
        ),
        (
            "active_switch",
            vec![
                "creator_field=other",
                "creation_date_field=made",
                "add_fields=true",
            ],
        ),
        (
            "active_zone",
            vec![
                "creator_field=maker",
                "creation_date_field=made",
                "record_dates_in=DATABASE_TIME",
            ],
        ),
    ] {
        let result = run(&dir, name, &args);
        assert!(result.status.success(), "{result:?}");
        assert!(
            snapshot(&dir) == original,
            "active configuration changed bytes"
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn native_table_without_tracking_elements_gets_valid_metadata() {
    use unigis_filegdb::GeodatabaseWriter;
    let dir = std::env::temp_dir().join(format!(
        "ug-enable-tracking-{}-native.gdb",
        std::process::id()
    ));
    let mut writer = GeodatabaseWriter::create(&dir).unwrap();
    let table = writer
        .create_table("native", None, false, false, &[], None)
        .unwrap();
    writer.finish_table(table).unwrap();
    writer.finish().unwrap();
    let mut args = ALL.to_vec();
    args.push("add_fields=true");
    let result = run(&dir, "native", &args);
    assert!(result.status.success(), "{result:?}");
    let gdb = Geodatabase::open(&dir).unwrap();
    let table = gdb.open_table("native").unwrap();
    assert_eq!(table.fields().len(), 4);
    for name in ["maker", "made", "changer", "changed"] {
        assert_eq!(table.field_editable(name), Some(false));
    }
    drop(table);
    drop(gdb);
    let first = snapshot(&dir);
    assert!(run(&dir, "native", &args).status.success());
    assert!(snapshot(&dir) == first);
    std::fs::remove_dir_all(dir).unwrap();
}
