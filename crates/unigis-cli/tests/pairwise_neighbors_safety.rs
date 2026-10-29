//! Unvalidated Pairwise options must fail before changing an existing table.
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let p = entry.unwrap().path();
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(p).unwrap(),
            )
        })
        .collect()
}

#[test]
fn unsupported_options_preserve_existing_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::temp_dir().join(format!("ug-ppn-safe-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let output = dir.join("existing.dbf");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "pairwise_polygon_neighbors"])
            .arg(format!(
                "input={}",
                root.join("testdata/geoproc/squares.shp").display()
            ))
            .arg(format!("output={}", output.display()))
            .args(args)
            .output()
            .unwrap()
    };
    let initial = run(&[]);
    assert!(initial.status.success(), "{initial:?}");
    let before = snapshot(&dir);
    for options in [
        vec!["precision=MAX_PRECISION"],
        vec!["cluster_tolerance=0.5 Meters"],
        vec!["out_linear_units=POINTS"],
        vec!["fields=NO_SUCH"],
        vec!["area_overlap=AREA_OVERLAP", "out_area_units=NO_SUCH"],
    ] {
        let result = run(&options);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(!result.stderr.is_empty());
        assert_eq!(snapshot(&dir), before, "{options:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
