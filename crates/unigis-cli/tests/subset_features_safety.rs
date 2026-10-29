//! New output admission, seeded partition identity and shared FileGDB publication.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};
use unigis_core::{FeatureClass, Value};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};
use unigis_shapefile::ShapefileReader;
fn source(item: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/geoproc/subset_base.gdb")
        .join(item)
}
fn temp(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-subset-{}-{label}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let e = e.unwrap();
            (!e.file_name().to_string_lossy().ends_with(".lock")).then(|| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read(e.path()).unwrap(),
                )
            })
        })
        .collect()
}
fn run(input: &Path, train: &Path, test: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "subset_features"])
        .arg(format!("input={}", input.display()))
        .arg(format!("out_training_feature_class={}", train.display()))
        .arg(format!("out_test_feature_class={}", test.display()))
        .args(extra)
        .output()
        .unwrap()
}
fn keys(fc: &dyn FeatureClass) -> BTreeSet<i32> {
    let i = fc.fields().iter().position(|f| f.name == "KEY").unwrap();
    fc.features()
        .map(|r| match r.unwrap().attributes[i] {
            Value::Integer(k) => k,
            _ => panic!("KEY type"),
        })
        .collect()
}
#[test]
fn seeded_folder_outputs_partition_all_records_without_source_changes() {
    let dir = temp("folder");
    let source = source("points");
    let before = snapshot(source.parent().unwrap());
    let mut seen = Vec::new();
    for (name, seed) in [("a", "seed=0"), ("repeat", "seed=0"), ("other", "seed=1")] {
        let train = dir.join(format!("{name}_train.shp"));
        let test = dir.join(format!("{name}_test.shp"));
        let r = run(&source, &train, &test, &[seed]);
        assert!(r.status.success(), "{r:?}");
        let a = keys(&ShapefileReader::open(train).unwrap());
        let b = keys(&ShapefileReader::open(test).unwrap());
        assert_eq!(a.len(), 4);
        assert_eq!(b.len(), 3);
        assert!(a.is_disjoint(&b));
        assert_eq!(
            a.union(&b).copied().collect::<Vec<_>>(),
            [1, 3, 4, 5, 6, 7, 8]
        );
        seen.push(a);
    }
    assert_eq!(seen[0], seen[1]);
    assert_ne!(seen[0], seen[2]);
    assert_eq!(snapshot(source.parent().unwrap()), before);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn existing_outputs_duplicate_names_and_bad_sizes_preserve_destination() {
    let dir = temp("refusals");
    let train = dir.join("existing.shp");
    for ext in ["shp", "shx", "dbf", "prj"] {
        std::fs::write(train.with_extension(ext), format!("keep {ext}")).unwrap();
    }
    let before = snapshot(&dir);
    for (a, b, args) in [
        (train.clone(), dir.join("new_test.shp"), vec![]),
        (dir.join("fresh.shp"), dir.join("FRESH.SHP"), vec![]),
        (
            dir.join("new.shp"),
            dir.join("new_test.shp"),
            vec!["size_of_training_dataset=-1"],
        ),
        (dir.join("new.shp"), dir.join("missing/test.shp"), vec![]),
    ] {
        let r = run(&source("points"), &a, &b, &args);
        assert!(!r.status.success(), "{r:?}");
        assert_eq!(snapshot(&dir), before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn feature_and_table_outputs_share_one_filegdb_and_keep_aliases_nulls() {
    let dir = temp("gdb");
    let out = dir.join("outputs.gdb");
    GeodatabaseWriter::create(&out).unwrap().finish().unwrap();
    for item in ["points", "records"] {
        let train = out.join(format!("{item}_train"));
        let test = out.join(format!("{item}_test"));
        let r = run(&source(item), &train, &test, &[]);
        assert!(r.status.success(), "{r:?}");
        let db = Geodatabase::open(&out).unwrap();
        let a = db.open_table(&format!("{item}_train")).unwrap();
        let b = db.open_table(&format!("{item}_test")).unwrap();
        let aa = keys(&a);
        let bb = keys(&b);
        assert_eq!(aa.len(), 4);
        assert_eq!(bb.len(), 3);
        assert!(aa.is_disjoint(&bb));
        assert_eq!(
            aa.union(&bb).copied().collect::<Vec<_>>(),
            [1, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(a.fields()[0].alias.as_deref(), Some("Record key"));
        let label = a.fields().iter().position(|f| f.name == "LABEL").unwrap();
        let key = a.fields().iter().position(|f| f.name == "KEY").unwrap();
        let rows = a
            .features()
            .chain(b.features())
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter()
                .find(|r| r.attributes[key] == Value::Integer(3))
                .unwrap()
                .attributes[label],
            Value::Null
        );
    }
    let before = snapshot(&out);
    let r = run(
        &source("points"),
        &out.join("new_training"),
        &out.join("points_test"),
        &[],
    );
    assert!(!r.status.success());
    assert_eq!(snapshot(&out), before);
    std::fs::remove_dir_all(dir).unwrap();
}
