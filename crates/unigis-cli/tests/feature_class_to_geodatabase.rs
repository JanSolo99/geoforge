//! Batch failure rollback and repeated naming preserve inputs and existing classes.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};
use unigis_filegdb::Geodatabase;
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn temp(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-fctg-{}-{label}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn copy(source: &Path, target: &Path) {
    std::fs::create_dir(target).unwrap();
    for e in std::fs::read_dir(source).unwrap() {
        let e = e.unwrap();
        if e.file_name().to_string_lossy().ends_with(".lock") {
            continue;
        }
        std::fs::copy(e.path(), target.join(e.file_name())).unwrap();
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
fn run(inputs: &str, target: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "feature_class_to_geodatabase"])
        .arg(format!("input_features={inputs}"))
        .arg(format!("output_geodatabase={}", target.display()))
        .current_dir(root())
        .output()
        .unwrap()
}
#[test]
fn invalid_batch_and_late_projection_failure_roll_back_destination() {
    let dir = temp("rollback");
    let out = dir.join("target.gdb");
    copy(&root().join("testdata/geoproc/fctg_seed.gdb"), &out);
    let before = snapshot(&out);
    for input in [
        "testdata/geoproc/wells.shp;testdata/geoproc/missing.shp",
        "testdata/geoproc/wells.shp;testdata/geoproc/globalids_base.gdb/records",
    ] {
        let r = run(input, &out);
        assert_eq!(r.status.code(), Some(1), "{r:?}");
        assert_eq!(snapshot(&out), before);
    }
    for ext in ["shp", "shx", "dbf"] {
        std::fs::copy(
            root().join(format!("testdata/geoproc/wells.{ext}")),
            dir.join(format!("bad.{ext}")),
        )
        .unwrap();
    }
    std::fs::write(dir.join("bad.prj"), "not a coordinate system").unwrap();
    let bad_source = snapshot(&out);
    let inputs = format!(
        "testdata/geoproc/roads.shp;{}",
        dir.join("bad.shp").display()
    );
    let r = run(&inputs, &out.join("group"));
    assert_eq!(r.status.code(), Some(1), "{r:?}");
    assert_eq!(snapshot(&out), bad_source);
    assert!(
        !Geodatabase::open(&out)
            .unwrap()
            .entries()
            .iter()
            .any(|e| e.name == "roads")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn same_workspace_import_allocates_names_without_changing_source_records() {
    let dir = temp("same");
    let out = dir.join("target.gdb");
    copy(&root().join("testdata/geoproc/fctg_seed.gdb"), &out);
    let db = Geodatabase::open(&out).unwrap();
    let source = db.open_table("parcels").unwrap();
    let original = source.features().collect::<Result<Vec<_>, _>>().unwrap();
    let source_base = db.table_base("parcels").unwrap();
    let prefix = format!("{}.", source_base.file_name().unwrap().to_string_lossy());
    let saved = snapshot(&out);
    drop(source);
    drop(db);
    let r = run(
        &format!(
            "'{}';'{}'",
            out.join("parcels").display(),
            out.join("parcels").display()
        ),
        &out,
    );
    assert!(r.status.success(), "{r:?}");
    let db = Geodatabase::open(&out).unwrap();
    for name in ["parcels", "parcels_1", "parcels_2"] {
        assert_eq!(
            db.open_table(name)
                .unwrap()
                .features()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            original
        );
    }
    let after = snapshot(&out);
    for (name, bytes) in saved {
        if name.starts_with(&prefix) {
            assert_eq!(after[&name], bytes);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
