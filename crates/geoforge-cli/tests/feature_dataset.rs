//! `gf create-feature-dataset` and tool outputs written into a feature dataset (`city.gdb/Ds/name`).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gf"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn outputs_go_into_a_feature_dataset() {
    let d = std::env::temp_dir().join(format!("gf-fds-cli-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let gdb = d.join("city.gdb");
    let g = gdb.to_str().unwrap();
    let out = gf(&["create-feature-dataset", g, "Transport", "32633"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let shp = root().join("testdata/shp/multipoint.shp");
    let into = gdb.join("Transport").join("stops");
    let out = gf(&[
        "copy-features",
        shp.to_str().unwrap(),
        into.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // Beside it, a class outside any dataset.
    let loose = gdb.join("loose");
    assert!(
        gf(&[
            "copy-features",
            shp.to_str().unwrap(),
            loose.to_str().unwrap()
        ])
        .status
        .success()
    );
    let db = Geodatabase::open(&gdb).unwrap();
    assert_eq!(db.feature_datasets(), ["Transport"]);
    let mut inside: Vec<_> = db
        .entries()
        .iter()
        .map(|e| (e.name.clone(), e.feature_dataset.clone()))
        .collect();
    inside.sort();
    assert_eq!(
        inside,
        [
            ("loose".into(), None),
            ("stops".into(), Some("Transport".into()))
        ]
    );
    // A dataset that is not there is an error, not a class dropped in the root.
    let bad = gdb.join("Nope").join("x");
    assert!(
        !gf(&[
            "copy-features",
            shp.to_str().unwrap(),
            bad.to_str().unwrap()
        ])
        .status
        .success()
    );
    // Deleting the dataset deletes its class.
    assert!(
        gf(&["delete", gdb.join("Transport").to_str().unwrap()])
            .status
            .success()
    );
    let db = Geodatabase::open(&gdb).unwrap();
    assert_eq!(db.entries().len(), 1);
    std::fs::remove_dir_all(&d).ok();
}
