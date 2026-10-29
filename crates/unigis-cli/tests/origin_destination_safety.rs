//! Unsupported link options must leave an existing output unchanged.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn snapshot(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(path).unwrap(),
            )
        })
        .collect()
}

#[test]
fn unsupported_links_preserve_existing_output() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let folder = std::env::temp_dir().join(format!("ug-od-safety-{}", std::process::id()));
    std::fs::create_dir(&folder).unwrap();
    let output = folder.join("existing.shp");
    let run = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "generate_origin_destination_links"])
            .arg(format!(
                "origin_features={}",
                root.join("testdata/geoproc/od_origins.shp").display()
            ))
            .arg(format!(
                "destination_features={}",
                root.join("testdata/geoproc/od_destinations.shp").display()
            ))
            .arg(format!("output={}", output.display()))
            .args(extra)
            .output()
            .unwrap()
    };
    let initial = run(&[]);
    assert!(initial.status.success(), "{initial:?}");
    let before = snapshot(&folder);
    for option in [
        "line_type=GEODESIC",
        "aggregate_links=AGGREGATE_OVERLAPPING",
        "sum_fields=NUM SUM",
        "origin_group_field=GRP",
        "num_nearest=NaN",
        "num_nearest=1.5",
        "num_nearest=-1",
        "search_distance=NaN",
        "search_distance=-1",
    ] {
        let result = run(&[option]);
        assert_eq!(result.status.code(), Some(1), "{option}: {result:?}");
        assert!(!result.stderr.is_empty());
        assert_eq!(snapshot(&folder), before, "{option}");
    }
    std::fs::remove_dir_all(folder).unwrap();
}
