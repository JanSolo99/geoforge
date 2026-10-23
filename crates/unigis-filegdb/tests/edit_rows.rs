//! Editing rows of a table in an ArcGIS-created geodatabase.

use std::path::{Path, PathBuf};

use unigis_core::{Feature, Value};
use unigis_filegdb::{Geodatabase, TableEditor};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

fn row(id: i32, name: &str) -> Feature {
    Feature {
        oid: 0,
        geometry: None,
        attributes: vec![Value::Integer(id), Value::String(name.into())],
    }
}

fn rows(gdb: &Path) -> Vec<(i64, Value, Value)> {
    Geodatabase::open(gdb)
        .unwrap()
        .open_table("t")
        .unwrap()
        .features()
        .map(|f| {
            let f = f.unwrap();
            (f.oid, f.attributes[0].clone(), f.attributes[1].clone())
        })
        .collect()
}

#[test]
fn insert_update_delete_rows_in_an_existing_table() {
    let dir = std::env::temp_dir().join(format!("gf-edit-{}", std::process::id()));
    let copy = dir.join("copy.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_base.gdb"), &copy);
    let before = rows(&copy);
    assert_eq!(before.len(), 100); // 101 rows less the deleted OBJECTID 7

    let base = copy.join("a0000000b");
    let mut ed = TableEditor::open(&base).unwrap();
    assert_eq!(ed.row_count(), 100);
    assert_eq!(ed.insert(&row(1000, "added a")).unwrap(), 102);
    assert_eq!(ed.insert(&row(1001, "added b")).unwrap(), 103);
    ed.update(5, &row(5, "longer replacement")).unwrap();
    ed.delete(10).unwrap();
    ed.delete(11).unwrap();
    assert!(ed.delete(7).is_err()); // already deleted
    assert_eq!(ed.finish().unwrap(), 100);

    let after = rows(&copy);
    assert_eq!(after.len(), 100);
    let get = |oid: i64| after.iter().find(|r| r.0 == oid).cloned();
    assert_eq!(get(102).unwrap().2, Value::String("added a".into()));
    assert_eq!(get(103).unwrap().1, Value::Integer(1001));
    assert_eq!(
        get(5).unwrap().2,
        Value::String("longer replacement".into())
    );
    assert!(get(10).is_none() && get(11).is_none());
    // Unchanged rows are unchanged.
    for r in &before {
        if ![5, 10, 11].contains(&r.0) {
            assert_eq!(get(r.0).as_ref(), Some(r));
        }
    }

    // The name index follows the edits: an index search finds the new rows.
    let table = Geodatabase::open(&copy).unwrap().open_table("t").unwrap();
    let hits = table
        .search_attribute(
            "NAME",
            &Value::String("added a".into()),
            &Value::String("added b".into()),
        )
        .unwrap();
    let oids: Vec<i64> = hits.iter().map(|f| f.oid).collect();
    assert_eq!(oids, vec![102, 103]);
    std::fs::remove_dir_all(&dir).ok();
}
