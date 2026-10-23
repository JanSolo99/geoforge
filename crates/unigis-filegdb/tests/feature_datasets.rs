//! Feature datasets written by UniGIS: items, paths and relationships as ArcGIS Pro writes them
//! (testdata/gdb-reference/catalog_ops/before.json: dataset `ds` with class `inds`), the class takes the
//! dataset's coordinate system, and GDAL / our reader see the class inside the dataset.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::{Feature, Field, FieldType, Geometry, GeometryKind, Value};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const UTM33: &str = r#"PROJCS["WGS_1984_UTM_Zone_33N",GEOGCS["GCS_WGS_1984",DATUM["D_WGS_1984",SPHEROID["WGS_1984",6378137.0,298.257223563]],PRIMEM["Greenwich",0.0],UNIT["Degree",0.0174532925199433]],PROJECTION["Transverse_Mercator"],PARAMETER["False_Easting",500000.0],PARAMETER["False_Northing",0.0],PARAMETER["Central_Meridian",15.0],PARAMETER["Scale_Factor",0.9996],PARAMETER["Latitude_Of_Origin",0.0],UNIT["Meter",1.0]]"#;

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gf-fds-{name}-{}", std::process::id()));
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

fn point(x: f64, y: f64) -> Feature {
    Feature {
        oid: 0,
        geometry: Some(Geometry::point(x, y)),
        attributes: vec![Value::Integer(1)],
    }
}

fn build(gdb: &Path) {
    let mut w = GeodatabaseWriter::create(gdb).unwrap();
    w.create_feature_dataset("ds", Some(UTM33)).unwrap();
    let fields = [Field::new("k", FieldType::Integer)];
    let mut t = w
        .create_feature_class_in("ds", "inds", GeometryKind::Point, false, false, &fields)
        .unwrap();
    t.write(&point(500_000.0, 5_000_000.0)).unwrap();
    w.finish_table(t).unwrap();
    let t = w
        .create_table(
            "loose",
            Some(GeometryKind::Point),
            false,
            false,
            &fields,
            Some(UTM33),
        )
        .unwrap();
    w.finish_table(t).unwrap();
    w.finish().unwrap();
}

#[test]
fn dataset_items_and_relationships_match_arcgis() {
    let gdb = tmp("new").join("a.gdb");
    build(&gdb);
    let g = Geodatabase::open(&gdb).unwrap();
    assert_eq!(g.feature_datasets(), ["ds"]);
    let inside: Vec<_> = g
        .entries()
        .iter()
        .map(|e| (e.name.as_str(), e.feature_dataset.as_deref()))
        .collect();
    assert_eq!(inside, [("inds", Some("ds")), ("loose", None)]);
    assert_eq!(g.open_table("inds").unwrap().row_count(), 1);
    // The class shares the dataset's coordinate system and domain.
    let (a, b) = (
        g.open_table("inds").unwrap(),
        g.open_table("loose").unwrap(),
    );
    let (da, db) = (a.geometry_def().unwrap(), b.geometry_def().unwrap());
    assert_eq!((da.x_origin, da.xy_scale), (db.x_origin, db.xy_scale));

    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let Some(ours) = snapshot(&gdb) else { return };
    let want: Json = serde_json::from_slice(
        &std::fs::read(root().join("testdata/gdb-reference/catalog_ops/before.json")).unwrap(),
    )
    .unwrap();
    // The relationships ArcGIS writes for a dataset and its class (the root folder is `\`).
    let rels: Vec<&str> = ours["rels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Json::as_str)
        .collect();
    let arcgis: Vec<&str> = want["rels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Json::as_str)
        .collect();
    for expect in [
        r"\ -> \ds : DC78F1AB",
        r"\ds -> \ds\inds : A1633A59",
        r"\ -> \loose : DC78F1AB",
    ] {
        assert!(rels.contains(&expect), "{expect} in {rels:?}");
        assert!(arcgis.contains(&expect), "{expect} in ArcGIS's {arcgis:?}");
    }
    assert!(
        !rels.contains(&r"\ -> \ds\inds : DC78F1AB"),
        "a class in a dataset is not in the root folder"
    );
    // Items: path, physical name, type of each.
    let items: Vec<(String, String, String)> = ours["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["path"].as_str().unwrap().to_string(),
                i["physical"].as_str().unwrap().to_string(),
                i["type"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    for want in [
        (r"\ds", "DS", "74737149"),
        (r"\ds\inds", "INDS", "70737809"),
        (r"\loose", "LOOSE", "70737809"),
    ] {
        assert!(
            items
                .iter()
                .any(|i| (i.0.as_str(), i.1.as_str(), i.2.as_str()) == want),
            "{want:?} in {items:?}"
        );
    }
    // GDAL sees the class (and its feature) inside the dataset.
    let out = Command::new("ogrinfo")
        .args(["-so", "-al"])
        .arg(&gdb)
        .output()
        .unwrap();
    let t = String::from_utf8_lossy(&out.stdout);
    assert!(
        t.contains("Layer name: inds")
            && t.contains("Feature Count: 1")
            && t.contains("UTM zone 33N"),
        "{t}"
    );
    std::fs::remove_dir_all(gdb.parent().unwrap()).ok();
}

#[test]
fn classes_join_a_dataset_of_an_existing_geodatabase() {
    let gdb = tmp("add").join("a.gdb");
    build(&gdb);
    let mut w = GeodatabaseWriter::open_existing(&gdb).unwrap();
    w.create_feature_dataset("ds2", None).unwrap();
    let fields = [Field::new("k", FieldType::Integer)];
    let t = w
        .create_feature_class_in("ds", "more", GeometryKind::Polygon, false, false, &fields)
        .unwrap();
    w.finish_table(t).unwrap();
    let t = w
        .create_feature_class_in(
            "DS2",
            "other",
            GeometryKind::Polyline,
            false,
            false,
            &fields,
        )
        .unwrap();
    w.finish_table(t).unwrap();
    assert!(
        w.create_feature_class_in("nope", "x", GeometryKind::Point, false, false, &fields)
            .is_err()
    );
    assert!(
        w.create_feature_dataset("DS", None).is_err(),
        "a name in use"
    );
    w.finish().unwrap();
    let g = Geodatabase::open(&gdb).unwrap();
    let mut inside: Vec<_> = g
        .entries()
        .iter()
        .map(|e| (e.name.clone(), e.feature_dataset.clone()))
        .collect();
    inside.sort();
    assert_eq!(
        inside,
        [
            ("inds".into(), Some("ds".into())),
            ("loose".into(), None),
            ("more".into(), Some("ds".into())),
            ("other".into(), Some("ds2".into()))
        ]
    );
    // Its XY domain is the dataset's, read back from the existing definition.
    let (a, b) = (g.open_table("inds").unwrap(), g.open_table("more").unwrap());
    assert_eq!(
        a.geometry_def().unwrap().x_origin,
        b.geometry_def().unwrap().x_origin
    );
    std::fs::remove_dir_all(gdb.parent().unwrap()).ok();
}

/// A projected coordinate system gets ArcGIS's own XY domain, not a fixed one
/// (testdata/gdb-reference/catalog_ops/before.gdb: UTM 33N dataset `ds`).
#[test]
fn projected_systems_get_arcgis_xy_domain() {
    let gdb = tmp("xy").join("a.gdb");
    build(&gdb);
    let ours = Geodatabase::open(&gdb).unwrap();
    let theirs =
        Geodatabase::open(root().join("testdata/gdb-reference/catalog_ops/before.gdb")).unwrap();
    let (a, b) = (
        ours.open_table("inds").unwrap(),
        theirs.open_table("inds").unwrap(),
    );
    let (da, db) = (a.geometry_def().unwrap(), b.geometry_def().unwrap());
    assert_eq!(
        (da.x_origin, da.y_origin, da.xy_scale, da.xy_tolerance),
        (db.x_origin, db.y_origin, db.xy_scale, db.xy_tolerance)
    );
    assert_eq!((da.x_origin, da.y_origin), (-5120900.0, -9998100.0));
    std::fs::remove_dir_all(gdb.parent().unwrap()).ok();
}

#[test]
fn xy_domain_table_has_what_arcgis_reported() {
    use unigis_filegdb::xy_domains::by_name;
    let d = by_name("WGS_1984_Web_Mercator_Auxiliary_Sphere").unwrap();
    assert_eq!(
        (d.x_origin, d.y_origin, d.scale, d.tolerance),
        (-20037700.0, -30241100.0, 10000.0, 0.001)
    );
    // Feet: a finer scale, in the system's unit.
    let d = by_name("NAD_1983_StatePlane_New_York_Long_Island_FIPS_3104_Feet").unwrap();
    assert_eq!((d.x_origin, d.y_origin), (-120039300.0, -96540300.0));
    assert!((d.scale - 3048.006096).abs() < 1e-3, "{}", d.scale);
    // Geographic systems: -400 and 1e9.
    let d = by_name("GCS_WGS_1984").unwrap();
    assert_eq!((d.x_origin, d.y_origin, d.scale), (-400.0, -400.0, 1e9));
    assert!(by_name("not a system").is_none());
}
