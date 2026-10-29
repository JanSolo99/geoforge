//! Workspace partitions must preserve existing files and preflight invalid input.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn existing_partitions_are_preserved_and_new_names_are_unique() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("ug-sba-existing-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "split_by_attributes", "split_fields=REGION"])
            .arg(format!(
                "input={}",
                root.join("testdata/geoproc/split_attributes.shp").display()
            ))
            .arg(format!("target_workspace={}", dir.display()))
            .output()
            .unwrap()
    };
    assert!(run().status.success());
    let old = snapshot(&dir);
    let second = run();
    assert!(second.status.success(), "{second:?}");
    assert!(dir.join("East0.shp").is_file() && dir.join("West0.shp").is_file());
    for (name, bytes) in old {
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), bytes);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_partitions_make_no_workspace_changes() {
    let root = std::env::temp_dir().join(format!("ug-sba-preflight-{}", std::process::id()));
    let folder = root.join("out");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("sentinel.bin"), b"keep existing data").unwrap();
    let input = root.join("data.csv");
    std::fs::write(
        &input,
        format!("REGION,CLASS\nGood,1\n{},2\n", "X".repeat(129)),
    )
    .unwrap();
    let before = snapshot(&folder);
    for field in ["REGION", "MISSING"] {
        let result = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "split_by_attributes"])
            .arg(format!("input={}", input.display()))
            .arg(format!("target_workspace={}", folder.display()))
            .arg(format!("split_fields={field}"))
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert_eq!(snapshot(&folder), before);
    }
    std::fs::remove_dir_all(root).unwrap();
}
