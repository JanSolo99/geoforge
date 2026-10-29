//! Invalid requests must leave an existing output family and its source untouched.
use std::{collections::BTreeMap, path::Path, process::Command};
fn snapshot(path: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(path)
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
fn invalid_requests_preserve_existing_output() {
    let dir = std::env::temp_dir().join(format!("ug-random-safety-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let out = dir.join("existing.shp");
    for ext in ["shp", "shx", "dbf", "prj"] {
        std::fs::write(out.with_extension(ext), format!("existing {ext}")).unwrap();
    }
    let before = snapshot(&dir);
    for args in [
        vec!["constraining_extent=0 0 0 10"],
        vec!["number_of_points_or_field=-1"],
        vec!["number_of_points_or_field=1000001"],
        vec!["minimum_allowed_distance=-1"],
        vec!["create_multipoint_output=MULTIPOINT", "multipoint_size=0"],
        vec!["seed=not-a-seed"],
    ] {
        let result = Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "create_random_points"])
            .arg(format!("output={}", out.display()))
            .args(args)
            .output()
            .unwrap();
        assert!(!result.status.success(), "{result:?}");
        assert_eq!(snapshot(&dir), before);
    }
    std::fs::remove_dir_all(dir).unwrap();
}
