//! Editor tracking edits retain user bytes; repeat/no-op/refusals are byte-identical.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use unigis_core::Value;
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
fn copy(name: &str) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/geoproc")
        .join(name);
    let temp = std::env::temp_dir().join(format!("ug-dtrack-{}-{name}", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), temp.join(e.file_name())).unwrap();
        }
    }
    temp
}
fn run(path: &Path, flags: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "disable_editor_tracking"])
        .arg(format!("input={}", path.display()))
        .args(flags)
        .output()
        .unwrap()
}
fn definition(dir: &Path, name: &str) -> String {
    let table = unigis_filegdb::Table::open(dir.join("a00000004")).unwrap();
    let col = |name: &str| table.fields().iter().position(|f| f.name == name).unwrap();
    let row = table
        .features()
        .map(Result::unwrap)
        .find(|f| matches!(&f.attributes[col("Name")],Value::String(n) if n==name))
        .unwrap();
    match &row.attributes[col("Definition")] {
        Value::Xml(x) | Value::String(x) => x.clone(),
        _ => panic!("missing definition"),
    }
}
fn assert_only_flag_bytes_changed(
    dir: &Path,
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
    group: &str,
    name: &str,
) {
    let gdb = unigis_filegdb::Geodatabase::open(dir).unwrap();
    let table_name = gdb
        .table_base(name)
        .unwrap()
        .with_extension("gdbtable")
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../../../scripts/research/disable_editor_tracking/field_flag_results.json"
    ))
    .unwrap();
    let fields = oracle[group]["before"][name].as_object().unwrap();
    let mut expected = before[&table_name].clone();
    for (field, location) in fields {
        let at = location["offset"].as_u64().unwrap() as usize;
        assert_eq!(expected[at], location["flags"].as_u64().unwrap() as u8);
        expected[at] = oracle[group]["after"][name][field]["flags"]
            .as_u64()
            .unwrap() as u8;
    }
    assert_eq!(
        after[&table_name], expected,
        "only observed field descriptor flag bytes may change"
    );
    for (file, bytes) in before {
        if file != &table_name && !file.starts_with("a00000004.") && !file.starts_with("a00000006.")
        {
            assert_eq!(
                after.get(file),
                Some(bytes),
                "user data/index/header changed: {file}"
            );
        }
    }
}
#[test]
fn catalog_only_idempotent_and_invalid_parameters_preserve_bytes() {
    let temp = copy("editor_tracking_base.gdb");
    let before = snapshot(&temp);
    let result = run(&temp.join("all_off"), &[]);
    assert!(result.status.success(), "{result:?}");
    let first = snapshot(&temp);
    assert_only_flag_bytes_changed(&temp, &before, &first, "base", "all_off");
    assert!(run(&temp.join("all_off"), &[]).status.success());
    assert_eq!(snapshot(&temp), first);
    assert!(
        run(
            &temp.join("keep_all"),
            &[
                "creator=false",
                "creation_date=false",
                "last_editor=false",
                "last_edit_date=false"
            ]
        )
        .status
        .success()
    );
    assert_eq!(snapshot(&temp), first);
    for (item, flags, error) in [
        ("creator_off", vec!["creator=garbage"], "000800"),
        ("missing", vec![], "000732"),
    ] {
        let result = run(&temp.join(item), &flags);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(error));
        assert_eq!(snapshot(&temp), first);
    }
    std::fs::remove_dir_all(temp).unwrap();
}
#[test]
fn feature_dataset_updates_members_and_retains_unrelated_metadata() {
    let temp = copy("editor_tracking_extra.gdb");
    let before = snapshot(&temp);
    let custom = definition(&temp, "custom");
    let plain = definition(&temp, "plain");
    let result = run(&temp.join("grouped"), &[]);
    assert!(result.status.success(), "{result:?}");
    let xml = definition(&temp, "tracked");
    let doc = roxmltree::Document::parse(&xml).unwrap();
    for (tag, expected) in [
        ("EditorTrackingEnabled", "false"),
        ("CreatorFieldName", ""),
        ("CreatedAtFieldName", ""),
        ("EditorFieldName", ""),
        ("EditedAtFieldName", ""),
    ] {
        assert_eq!(
            doc.root_element()
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == tag)
                .unwrap()
                .text()
                .unwrap_or(""),
            expected
        );
    }
    assert_eq!(definition(&temp, "custom"), custom);
    assert_eq!(definition(&temp, "plain"), plain);
    let first = snapshot(&temp);
    assert_only_flag_bytes_changed(&temp, &before, &first, "extra", "tracked");
    assert!(run(&temp.join("grouped"), &[]).status.success());
    assert_eq!(snapshot(&temp), first);
    std::fs::remove_dir_all(temp).unwrap();
}
