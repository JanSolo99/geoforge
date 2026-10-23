//! Editing a feature class of an ArcGIS-created geodatabase: geometry changes keep the
//! spatial index (and the stored extent) in step.

use std::path::{Path, PathBuf};

use unigis_core::{Envelope, Feature, Geometry, Value};
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

fn well(id: i32, x: f64, y: f64) -> Feature {
    Feature {
        oid: 0,
        geometry: Some(Geometry::point(x, y)),
        attributes: vec![Value::Integer(id), Value::Double(12.5)],
    }
}

fn window(x: f64, y: f64, r: f64) -> Envelope {
    Envelope {
        xmin: x - r,
        ymin: y - r,
        xmax: x + r,
        ymax: y + r,
        z: None,
        m: None,
    }
}

#[test]
fn spatial_index_follows_feature_edits() {
    let dir = std::env::temp_dir().join(format!("gf-edit-fc-{}", std::process::id()));
    let copy = dir.join("copy.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &copy);
    let gdb = Geodatabase::open(&copy).unwrap();
    let base = gdb.table_base("wells").unwrap();
    let ids_in = |w: &Envelope| -> Vec<i64> {
        let mut v: Vec<i64> = Geodatabase::open(&copy)
            .unwrap()
            .open_table("wells")
            .unwrap()
            .features_in_extent(w)
            .unwrap()
            .iter()
            .map(|f| f.oid)
            .collect();
        v.sort_unstable();
        v
    };
    // Far from every existing well.
    let far = (510_000.0, 5_010_000.0);
    assert!(ids_in(&window(far.0, far.1, 100.0)).is_empty());

    let mut ed = TableEditor::open(&base).unwrap();
    let a = ed.insert(&well(900, far.0, far.1)).unwrap();
    let b = ed.insert(&well(901, far.0 + 50.0, far.1 + 50.0)).unwrap();
    ed.update(1, &well(1, far.0 - 30.0, far.1)).unwrap(); // moved far away
    ed.delete(2).unwrap();
    ed.finish().unwrap();

    assert_eq!(
        ids_in(&window(far.0, far.1, 100.0)),
        vec![1, a as i64, b as i64]
    );
    let moved_from = Geodatabase::open(&copy)
        .unwrap()
        .open_table("wells")
        .unwrap();
    assert_eq!(moved_from.row_count(), 51); // 50 + 2 inserted - 1 deleted
    // The stored extent grew to include the new wells.
    let ext = moved_from.geometry_def().unwrap().extent;
    assert!(ext.xmax >= far.0 + 50.0 && ext.ymax >= far.1 + 50.0);
    std::fs::remove_dir_all(&dir).ok();
}
