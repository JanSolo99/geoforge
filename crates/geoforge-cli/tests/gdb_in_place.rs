//! Tools that edit their input work on a geodatabase table in place: the OBJECTIDs,
//! the other rows and the geometry survive, and the new field is readable.

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::Value;
use geoforge_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) {
    let out = Command::new(env!("CARGO_BIN_EXE_gf"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

#[test]
fn add_field_and_calculate_field_edit_a_geodatabase_table() {
    let dir = std::env::temp_dir().join(format!("gf-gdb-inplace-{}", std::process::id()));
    let gdb = dir.join("edit_fc.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &gdb);
    let fc = gdb.join("wells");
    let fc = fc.to_str().unwrap();
    let before = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("wells")
        .unwrap();
    let before: Vec<_> = before.features().map(|f| f.unwrap()).collect();

    gf(&["add-field", fc, "NOTE", "TEXT", "--length", "10"]);
    gf(&["calculate-field", fc, "NOTE", "\"w\" + str(!WELL_ID!)"]);

    let table = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("wells")
        .unwrap();
    let after: Vec<_> = table.features().map(|f| f.unwrap()).collect();
    assert_eq!(after.len(), before.len());
    for (a, b) in after.iter().zip(&before) {
        assert_eq!(a.oid, b.oid);
        assert_eq!(a.geometry, b.geometry);
        assert_eq!(a.attributes[..2], b.attributes[..2]);
        let id = &a.attributes[0];
        let Value::Integer(id) = id else {
            panic!("{id:?}")
        };
        assert_eq!(a.attributes[2], Value::String(format!("w{id}")));
    }
    std::fs::remove_dir_all(&dir).ok();
}
