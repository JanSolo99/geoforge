//! `ug delete` / `ug rename` on shapefiles (all sidecar files, nothing else) and geodatabase items.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use unigis_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gf-delren-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn shapefiles_move_and_vanish_with_their_sidecars() {
    let d = tmp("shp");
    for ext in ["shp", "shx", "dbf", "prj", "cpg"] {
        std::fs::copy(
            root().join(format!("testdata/shp/multipoint.{ext}")),
            d.join(format!("a.{ext}")),
        )
        .unwrap();
    }
    std::fs::write(d.join("a.txt"), "notes").unwrap(); // not a shapefile file: left alone
    let (a, b) = (d.join("a.shp"), d.join("b.shp"));
    assert!(gf(&["rename", a.to_str().unwrap(), "b"]).status.success());
    assert_eq!(
        names(&d),
        ["a.txt", "b.cpg", "b.dbf", "b.prj", "b.shp", "b.shx"]
    );
    // A name taken is refused.
    std::fs::copy(d.join("b.shp"), d.join("c.shp")).unwrap();
    let out = gf(&["rename", b.to_str().unwrap(), "c"]);
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("000725"));
    assert!(gf(&["delete", b.to_str().unwrap()]).status.success());
    assert_eq!(names(&d), ["a.txt", "c.shp"]);
    let out = gf(&["delete", b.to_str().unwrap()]);
    assert!(!out.status.success() && String::from_utf8_lossy(&out.stderr).contains("000732"));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn geodatabase_items() {
    let d = tmp("gdb");
    let g = d.join("c.gdb");
    std::fs::create_dir_all(&g).unwrap();
    for e in std::fs::read_dir(root().join("testdata/gdb-reference/catalog_ops/before.gdb"))
        .unwrap()
        .flatten()
    {
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), g.join(e.file_name())).unwrap();
        }
    }
    let item = |n: &str| g.join(n).to_str().unwrap().to_owned();
    assert!(gf(&["rename", &item("tbl"), "tbl2"]).status.success());
    assert!(
        gf(&["alter-domain", g.to_str().unwrap(), "dom", "dom2"])
            .status
            .success()
    );
    let out = gf(&["delete", &item("tbl2")]);
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("tbl2, rc"));
    let gdb = Geodatabase::open(&g).unwrap();
    let mut left: Vec<&str> = gdb.entries().iter().map(|e| e.name.as_str()).collect();
    left.sort();
    assert_eq!(left, ["inds", "loose"]);
    assert_eq!(gdb.domains().len(), 1);
    assert_eq!(gdb.domains()[0].name, "dom2");
    std::fs::remove_dir_all(&d).ok();
}
