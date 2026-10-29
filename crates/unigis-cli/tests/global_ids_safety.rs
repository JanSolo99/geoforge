//! Batch admission, rollback, idempotence and GlobalID metadata.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_filegdb::{Geodatabase, add_global_ids};
fn copy(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-globalids-{}-{label}.gdb", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    let src =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/geoproc/globalids_base.gdb");
    for (n, b) in snapshot(&src) {
        std::fs::write(p.join(n), b).unwrap();
    }
    p
}
fn snapshot(p: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(p)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| !e.file_name().to_string_lossy().ends_with(".lock"))
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}
#[test]
fn batch_refusals_preserve_bytes_and_repeat_is_a_noop() {
    let p = copy("batch");
    let before = snapshot(&p);
    assert!(add_global_ids(&p, &["points".into(), "missing".into()]).is_err());
    assert_eq!(before, snapshot(&p));
    add_global_ids(&p, &["points".into(), "points".into(), "group".into()]).unwrap();
    let after = snapshot(&p);
    add_global_ids(&p, &["points".into(), "group".into()]).unwrap();
    assert_eq!(after, snapshot(&p));
    let g = Geodatabase::open(&p).unwrap();
    for name in ["points", "child"] {
        let t = g.open_table(name).unwrap();
        assert_eq!(t.field_editable("GlobalID"), Some(false));
    }
    drop(g);
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn journal_rolls_back_after_an_earlier_table_was_changed() {
    let p = copy("rollback");
    // A malformed second table passes catalog expansion, then fails after editing points.
    let base = Geodatabase::open(&p)
        .unwrap()
        .table_base("records")
        .unwrap();
    std::fs::write(base.with_extension("gdbtable"), b"broken").unwrap();
    let before = snapshot(&p);
    assert!(add_global_ids(&p, &["points".into(), "records".into()]).is_err());
    assert_eq!(before, snapshot(&p));
    std::fs::remove_dir_all(p).unwrap();
}
#[test]
fn cli_refuses_cross_workspace_before_mutation() {
    let a = copy("cross-a");
    let b = copy("cross-b");
    let old_a = snapshot(&a);
    let old_b = snapshot(&b);
    let result = Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "add_global_ids"])
        .arg(format!(
            "in_datasets={};{}",
            a.join("points").display(),
            b.join("records").display()
        ))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("share one FileGDB"));
    assert_eq!(old_a, snapshot(&a));
    assert_eq!(old_b, snapshot(&b));
    std::fs::remove_dir_all(a).unwrap();
    std::fs::remove_dir_all(b).unwrap();
}
