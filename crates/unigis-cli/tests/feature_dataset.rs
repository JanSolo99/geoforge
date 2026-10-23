//! `ug create-feature-dataset` and tool outputs written into a feature dataset (`city.gdb/Ds/name`).

use std::path::{Path, PathBuf};
use std::process::Command;

use unigis_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
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

/// An output in another coordinate system is projected to the dataset's, as ArcGIS does.
#[test]
fn outputs_are_projected_to_the_datasets_system() {
    let d = std::env::temp_dir().join(format!("gf-fds-proj-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    let gdb = d.join("c.gdb");
    assert!(
        gf(&[
            "create-feature-dataset",
            gdb.to_str().unwrap(),
            "Utm",
            "32633"
        ])
        .status
        .success()
    );
    let shp = root().join("testdata/shp/point.shp");
    let into = gdb.join("Utm").join("pts");
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
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("projected"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let db = Geodatabase::open(&gdb).unwrap();
    let t = db.open_table("pts").unwrap();
    let g = t.features().next().unwrap().unwrap().geometry.unwrap();
    // The source's first point (lon, lat) through PROJ.
    let src = Command::new("ogrinfo")
        .args(["-al", "-geom=WKT"])
        .arg(&shp)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&src.stdout).into_owned();
    let wkt = text
        .lines()
        .find(|l| l.trim_start().starts_with("POINT"))
        .unwrap();
    let coords: Vec<f64> = wkt
        .trim()
        .trim_start_matches("POINT")
        .trim()
        .trim_matches(|c| c == '(' || c == ')')
        .split_whitespace()
        .filter_map(|v| v.parse().ok())
        .collect();
    let from = unigis_proj::Crs::parse("4326").unwrap();
    let to = unigis_proj::Crs::parse("32633").unwrap();
    let mut xy = [[coords[0], coords[1]]];
    unigis_proj::Transform::with_method(&from, &to, None)
        .unwrap()
        .apply(&mut xy)
        .unwrap();
    assert!(
        (g.xy[0][0] - xy[0][0]).abs() < 0.01 && (g.xy[0][1] - xy[0][1]).abs() < 0.01,
        "{:?} vs {xy:?}",
        g.xy[0]
    );
    assert!(g.xy[0][0].abs() > 1000.0, "metres, not degrees");
    std::fs::remove_dir_all(&d).ok();
}
