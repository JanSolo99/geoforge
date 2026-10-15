//! Rewriting a table of an ArcGIS-created geodatabase with a different set of fields.

use std::path::{Path, PathBuf};

use geoforge_core::{Envelope, Feature, Field, FieldType, Value};
use geoforge_filegdb::{Geodatabase, rewrite_table};

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

#[test]
fn add_and_drop_fields_keeping_object_ids() {
    let dir = std::env::temp_dir().join(format!("gf-rewrite-{}", std::process::id()));
    let copy = dir.join("copy.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_base.gdb"), &copy);
    let gdb = Geodatabase::open(&copy).unwrap();
    let base = gdb.table_base("t").unwrap();
    let before: Vec<Feature> = gdb
        .open_table("t")
        .unwrap()
        .features()
        .map(|f| f.unwrap())
        .collect();
    assert_eq!(before.len(), 100);

    // Fields ID (kept), NAME (kept), VAL (new): rows keep their OBJECTIDs, a new row is added.
    let fields = vec![
        Field::new("ID", FieldType::Integer),
        {
            let mut f = Field::new("NAME", FieldType::String);
            f.length = 20;
            f
        },
        Field::new("VAL", FieldType::Double),
    ];
    let mut rows: Vec<(Option<u64>, Feature)> = before
        .iter()
        .map(|f| {
            let mut g = f.clone();
            g.attributes.push(Value::Double(f.oid as f64 / 2.0));
            (Some(f.oid as u64), g)
        })
        .collect();
    rows.push((
        None,
        Feature {
            oid: 0,
            geometry: None,
            attributes: vec![
                Value::Integer(7777),
                Value::String("new".into()),
                Value::Double(1.5),
            ],
        },
    ));
    assert_eq!(rewrite_table(&base, &fields, &rows).unwrap(), 101);

    let after: Vec<Feature> = Geodatabase::open(&copy)
        .unwrap()
        .open_table("t")
        .unwrap()
        .features()
        .map(|f| f.unwrap())
        .collect();
    assert_eq!(after.len(), 101);
    assert!(after.iter().all(|f| f.oid != 7), "OBJECTID 7 stays deleted");
    assert_eq!(
        after.last().unwrap().oid,
        102,
        "new rows follow the largest OBJECTID"
    );
    assert_eq!(after[0].attributes[2], Value::Double(0.5));
    // The index on NAME survived and still finds rows.
    let t = Geodatabase::open(&copy).unwrap().open_table("t").unwrap();
    let hits = t
        .search_attribute(
            "NAME",
            &Value::String("new".into()),
            &Value::String("new".into()),
        )
        .unwrap();
    assert_eq!(hits.len(), 1);

    // Dropping NAME drops its index.
    let fields = vec![Field::new("ID", FieldType::Integer)];
    let rows: Vec<(Option<u64>, Feature)> = after
        .iter()
        .map(|f| {
            let mut g = f.clone();
            g.attributes.truncate(1);
            (Some(f.oid as u64), g)
        })
        .collect();
    rewrite_table(&base, &fields, &rows).unwrap();
    assert!(!base.with_extension("idx_name.atx").exists());
    assert_eq!(
        Geodatabase::open(&copy)
            .unwrap()
            .open_table("t")
            .unwrap()
            .fields()
            .len(),
        1
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn feature_class_keeps_geometry_and_spatial_index() {
    let dir = std::env::temp_dir().join(format!("gf-rewrite-fc-{}", std::process::id()));
    let copy = dir.join("copy.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &copy);
    let gdb = Geodatabase::open(&copy).unwrap();
    let base = gdb.table_base("wells").unwrap();
    let t = gdb.open_table("wells").unwrap();
    let before: Vec<Feature> = t.features().map(|f| f.unwrap()).collect();
    let mut fields = t.fields().to_vec();
    fields.push(Field::new("EXTRA", FieldType::Integer));
    let rows: Vec<(Option<u64>, Feature)> = before
        .iter()
        .filter(|f| f.oid != 3)
        .map(|f| {
            let mut g = f.clone();
            g.attributes.push(Value::Integer(5));
            (Some(f.oid as u64), g)
        })
        .collect();
    rewrite_table(&base, &fields, &rows).unwrap();

    let t = Geodatabase::open(&copy)
        .unwrap()
        .open_table("wells")
        .unwrap();
    assert_eq!(t.row_count(), 49);
    assert_eq!(t.fields().len(), fields.len());
    let all = Envelope {
        xmin: 0.0,
        ymin: 0.0,
        xmax: 1e9,
        ymax: 1e9,
        z: None,
        m: None,
    };
    let found = t.features_in_extent(&all).unwrap();
    assert_eq!(found.len(), 49);
    let g0 = before[0].geometry.as_ref().unwrap();
    assert_eq!(found[0].geometry.as_ref().unwrap().xy, g0.xy);
    std::fs::remove_dir_all(&dir).ok();
}
