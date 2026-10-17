//! Relationship classes written by GeoForge read back, and link their classes, as ArcGIS Pro's do
//! (testdata/gdb-reference/catalog_ops/before.gdb: `rc`, one to many from `tbl` to `loose` on `k`).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::{Field, FieldType, GeometryKind};
use geoforge_filegdb::schema::{Cardinality, RelationshipClass};
use geoforge_filegdb::{Geodatabase, GeodatabaseWriter};
use serde_json::Value as Json;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rc(name: &str, origin: &str, destination: &str) -> RelationshipClass {
    RelationshipClass {
        name: name.into(),
        cardinality: Cardinality::OneToMany,
        is_composite: false,
        is_attributed: false,
        notification: "None".into(),
        origin: origin.into(),
        destination: destination.into(),
        origin_primary_key: "k".into(),
        origin_foreign_key: "k".into(),
        destination_primary_key: None,
        destination_foreign_key: None,
        forward_label: "f".into(),
        backward_label: "b".into(),
    }
}

fn build(gdb: &Path) {
    let mut w = GeodatabaseWriter::create(gdb).unwrap();
    let fields = [Field::new("k", FieldType::Integer)];
    let t = w
        .create_table("tbl", None, false, false, &fields, None)
        .unwrap();
    w.finish_table(t).unwrap();
    let t = w
        .create_table(
            "loose",
            Some(GeometryKind::Polygon),
            false,
            false,
            &fields,
            None,
        )
        .unwrap();
    w.finish_table(t).unwrap();
    w.add_relationship_class(&rc("rc", "tbl", "loose")).unwrap();
    w.finish().unwrap();
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gf-rc-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&d).ok();
    d
}

fn snapshot(gdb: &Path) -> Option<Json> {
    let out = Command::new("python3")
        .arg(root().join("scripts/gdb-snapshot.py"))
        .arg(gdb)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| serde_json::from_slice(&out.stdout).ok())?
}

#[test]
fn relationship_class_reads_back_and_links_its_classes_like_arcgis() {
    let gdb = tmp("new").join("a.gdb");
    build(&gdb);
    let ours = Geodatabase::open(&gdb).unwrap();
    let theirs =
        Geodatabase::open(root().join("testdata/gdb-reference/catalog_ops/before.gdb")).unwrap();
    assert_eq!(ours.relationship_classes(), theirs.relationship_classes());
    assert_eq!(ours.relationship_classes().len(), 1);
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let Some(snap) = snapshot(&gdb) else { return };
    let want = snapshot(&root().join("testdata/gdb-reference/catalog_ops/before.gdb")).unwrap();
    let rels = |s: &Json| -> Vec<String> {
        s["rels"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_str)
            .filter(|r| r.contains(r"\rc"))
            .map(String::from)
            .collect()
    };
    let (mut a, mut b) = (rels(&snap), rels(&want));
    a.sort();
    b.sort();
    assert_eq!(
        a, b,
        "the relationship class is in the root folder and related through both classes"
    );
    assert_eq!(a.len(), 3);
    let item = |s: &Json| {
        s["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == "rc")
            .map(|i| (i["path"].clone(), i["physical"].clone(), i["type"].clone()))
    };
    assert_eq!(item(&snap), item(&want));
    // GDAL reads it as a relationship.
    let out = Command::new("ogrinfo")
        .arg("-so")
        .arg(&gdb)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Relationship: rc (Association, tbl, loose)"),
        "{text}"
    );
    std::fs::remove_dir_all(gdb.parent().unwrap()).ok();
}

#[test]
fn relationship_classes_join_classes_of_an_existing_geodatabase_and_are_checked() {
    let gdb = tmp("add").join("a.gdb");
    build(&gdb);
    let mut w = GeodatabaseWriter::open_existing(&gdb).unwrap();
    let mut comp = rc("comp", "tbl", "loose");
    comp.is_composite = true;
    comp.notification = "Forward".into();
    comp.cardinality = Cardinality::OneToOne;
    // ArcGIS refuses a second class of the same name; so do we.
    assert!(w.add_relationship_class(&rc("RC", "tbl", "loose")).is_err());
    w.add_relationship_class(&comp).unwrap();
    // Missing classes and keys, and kinds not written yet.
    assert!(
        w.add_relationship_class(&rc("x1", "nope", "loose"))
            .is_err()
    );
    let mut bad_key = rc("x2", "tbl", "loose");
    bad_key.origin_foreign_key = "nokey".into();
    assert!(w.add_relationship_class(&bad_key).is_err());
    let mut mn = rc("x3", "tbl", "loose");
    mn.cardinality = Cardinality::ManyToMany;
    assert!(w.add_relationship_class(&mn).is_err());
    w.finish().unwrap();
    let g = Geodatabase::open(&gdb).unwrap();
    let all: Vec<_> = g
        .relationship_classes()
        .iter()
        .map(|r| {
            (
                r.name.as_str(),
                r.cardinality,
                r.is_composite,
                r.notification.as_str(),
            )
        })
        .collect();
    assert_eq!(
        all,
        [
            ("comp", Cardinality::OneToOne, true, "Forward"),
            ("rc", Cardinality::OneToMany, false, "None")
        ]
    );
    // Deleting a class takes the relationship classes using it along.
    let gone = geoforge_filegdb::delete_item(&gdb, "loose").unwrap();
    assert_eq!(gone.len(), 3, "{gone:?}");
    assert!(
        Geodatabase::open(&gdb)
            .unwrap()
            .relationship_classes()
            .is_empty()
    );
    std::fs::remove_dir_all(gdb.parent().unwrap()).ok();
}

/// `GF_OUT=/path/x.gdb cargo test -p geoforge-filegdb --test relationship_classes -- --ignored`
#[test]
#[ignore]
fn write_for_arcgis() {
    let out = std::env::var("GF_OUT").expect("GF_OUT");
    std::fs::remove_dir_all(&out).ok();
    build(Path::new(&out));
    let mut w = GeodatabaseWriter::open_existing(&out).unwrap();
    let mut comp = rc("comp", "tbl", "loose");
    comp.is_composite = true;
    comp.notification = "Forward".into();
    comp.forward_label = "owns".into();
    w.add_relationship_class(&comp).unwrap();
    w.finish().unwrap();
}
