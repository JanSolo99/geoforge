//! Disable COGO must retain all user data and be an idempotent metadata edit.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
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
fn disables_metadata_without_rewriting_user_data_and_repeat_is_byte_identical() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/cogo_base.gdb");
    let temp = std::env::temp_dir().join(format!("ug-disable-cogo-{}.gdb", std::process::id()));
    std::fs::create_dir(&temp).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), temp.join(e.file_name())).unwrap();
        }
    }
    let before = snapshot(&temp);
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "disable_cogo"])
            .arg(format!("input={}/enabled", temp.display()))
            .output()
            .unwrap()
    };
    let result = run();
    assert!(result.status.success(), "{result:?}");
    let first = snapshot(&temp);
    for (name, bytes) in &before {
        if !name.starts_with("a00000004.") && !name.starts_with("a00000006.") {
            assert_eq!(
                first.get(name),
                Some(bytes),
                "user data/index/header changed: {name}"
            );
        }
    }
    assert!(run().status.success());
    assert_eq!(snapshot(&temp), first);
    // A wrong geometry must fail without altering any remaining catalog/data byte.
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "disable_cogo"])
        .arg(format!("input={}/point", temp.display()))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("003917"));
    assert_eq!(snapshot(&temp), first);
    std::fs::remove_dir_all(temp).unwrap();
}
