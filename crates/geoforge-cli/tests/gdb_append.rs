//! `ug` adds its outputs to a geodatabase that already exists: one written by an earlier
//! `ug` run, and one ArcGIS created.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&std::ffi::OsStr]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn outputs_accumulate_in_one_geodatabase() {
    let dir = std::env::temp_dir().join(format!("gf-gdb-append-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let gdb = dir.join("out.gdb");
    let data = |n: &str| root().join(format!("testdata/geoproc/{n}.shp"));
    let run = |args: Vec<std::ffi::OsString>| {
        let refs: Vec<&std::ffi::OsStr> = args.iter().map(|a| a.as_os_str()).collect();
        let out = gf(&refs);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    };
    run(vec![
        "buffer".into(),
        data("wells").into(),
        gdb.join("wells_buf").into(),
        "30".into(),
    ]);
    run(vec![
        "copy-features".into(),
        data("parcels").into(),
        gdb.join("parcels").into(),
    ]);
    run(vec![
        "frequency".into(),
        data("parcels").into(),
        gdb.join("zones").into(),
        "ZONE".into(),
    ]);
    // The same name twice is refused.
    let again = gf(&[
        "copy-features".as_ref(),
        data("parcels").as_os_str(),
        gdb.join("parcels").as_os_str(),
    ]);
    assert!(!again.status.success());

    let g = Geodatabase::open(&gdb).unwrap();
    let names: Vec<String> = g.entries().iter().map(|e| e.name.clone()).collect();
    for n in ["wells_buf", "parcels", "zones"] {
        assert!(names.iter().any(|x| x == n), "{n} missing from {names:?}");
    }
    assert_eq!(g.open_table("wells_buf").unwrap().row_count(), 50);
    assert_eq!(g.open_table("parcels").unwrap().row_count(), 36);
    assert_eq!(g.open_table("zones").unwrap().row_count(), 4);

    // Into a geodatabase ArcGIS created (system tables with indexes).
    let arcgis = dir.join("arcgis.gdb");
    std::fs::create_dir_all(&arcgis).unwrap();
    for e in std::fs::read_dir(root().join("testdata/gdb-reference/edit_base.gdb")).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), arcgis.join(e.file_name())).unwrap();
    }
    run(vec![
        "copy-features".into(),
        data("wells").into(),
        arcgis.join("wells").into(),
    ]);
    let g = Geodatabase::open(&arcgis).unwrap();
    assert_eq!(g.open_table("wells").unwrap().row_count(), 50);
    assert_eq!(g.open_table("t").unwrap().row_count(), 100); // untouched
    if Command::new("ogrinfo").arg("--version").output().is_ok() {
        let o = Command::new("ogrinfo")
            .arg("-ro")
            .arg(&arcgis)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&o.stdout);
        assert!(text.contains("wells") && text.contains(": t ("), "{text}");
    }
    std::fs::remove_dir_all(&dir).ok();
}
