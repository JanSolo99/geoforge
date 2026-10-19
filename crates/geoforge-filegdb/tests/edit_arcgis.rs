//! Row and field edits of a geodatabase ArcGIS Pro 3.7 wrote
//! (`testdata/gdb-reference/edit_arcgis.gdb`: polygons with `Shape_Length`/`Shape_Area`
//! and an attribute index, Z polylines, a 64-bit OBJECTID point class and a table, all
//! with deleted rows and `.freelist`s). After the edits GDAL must read every table the way
//! we do, and the indexes, measures and OBJECTIDs must follow ArcGIS's rules.

use std::path::{Path, PathBuf};

use geoforge_core::{Envelope, Feature, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_filegdb::{
    Geodatabase, TableEditor, add_field, delete_fields, delete_rows, truncate_table,
};
use geoforge_testkit::{compare_feature, gdal_version, ogr_layer, ogrinfo_available};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::remove_dir_all(to).ok();
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

fn square(x: f64, y: f64, side: f64) -> Geometry {
    // Counter-clockwise on purpose: the writer orients shells the Esri way.
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polygon;
    g.xy = vec![
        [x, y],
        [x + side, y],
        [x + side, y + side],
        [x, y + side],
        [x, y],
    ];
    g.parts = vec![0];
    g
}

fn line_z(pts: &[[f64; 3]]) -> Geometry {
    let mut g = Geometry::point(0.0, 0.0);
    g.kind = GeometryKind::Polyline;
    g.xy = pts.iter().map(|p| [p[0], p[1]]).collect();
    g.z = Some(pts.iter().map(|p| p[2]).collect());
    g.parts = vec![0];
    g
}

fn window(xmin: f64, ymin: f64, xmax: f64, ymax: f64) -> Envelope {
    Envelope {
        xmin,
        ymin,
        xmax,
        ymax,
        z: None,
        m: None,
    }
}

fn rows(gdb: &Path, name: &str) -> Vec<Feature> {
    Geodatabase::open(gdb)
        .unwrap()
        .open_table(name)
        .unwrap()
        .features()
        .map(|f| f.unwrap())
        .collect()
}

/// Differences between our reading of `name` and GDAL's.
fn gdal_diffs(gdb: &Path, name: &str) -> Vec<String> {
    let table = Geodatabase::open(gdb).unwrap().open_table(name).unwrap();
    let gdal = ogr_layer(gdb, Some(name)).unwrap();
    let names: Vec<&str> = table.fields().iter().map(|f| f.name.as_str()).collect();
    let gdal_names: Vec<&str> = gdal.fields.iter().map(|f| f.name.as_str()).collect();
    let mut diffs = Vec::new();
    if names != gdal_names {
        diffs.push(format!("fields: ours {names:?}, GDAL {gdal_names:?}"));
    }
    let mut n = 0;
    for f in table.features() {
        let f = f.unwrap();
        n += 1;
        match gdal.features.get(&f.oid) {
            Some(g) => diffs.extend(compare_feature(&f, &names, g)),
            None => diffs.push(format!("OBJECTID {} not read by GDAL", f.oid)),
        }
    }
    if n != gdal.features.len() {
        diffs.push(format!("count: ours {n}, GDAL {}", gdal.features.len()));
    }
    diffs
}

fn double(v: &Value) -> f64 {
    match v {
        Value::Double(d) => *d,
        other => panic!("{other:?}"),
    }
}

#[test]
fn edit_every_kind_of_table_arcgis_wrote() {
    let dir = std::env::temp_dir().join(format!("gf-edit-arcgis-{}", std::process::id()));
    let gdb = dir.join("edit_arcgis.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_arcgis.gdb"), &gdb);
    let g = Geodatabase::open(&gdb).unwrap();
    let base = |n: &str| g.table_base(n).unwrap();

    // parcels: polygons, fields Shape_Length, Shape_Area, NAME (indexed), CLASS, VALUE,
    // SURVEYED; OBJECTIDs 1..20 with 4 and 9 deleted.
    let before = rows(&gdb, "parcels");
    assert_eq!(before.len(), 18);
    let mut ed = TableEditor::open(base("parcels")).unwrap();
    let names: Vec<String> = ed.fields().iter().map(|f| f.name.clone()).collect();
    assert_eq!(
        names,
        [
            "Shape_Length",
            "Shape_Area",
            "NAME",
            "CLASS",
            "VALUE",
            "SURVEYED"
        ]
    );
    let new = |name: &str, side: f64, x: f64| Feature {
        oid: 0,
        geometry: Some(square(x, 449_000.0, side)),
        // Wrong measures on purpose: the editor computes them.
        attributes: vec![
            Value::Double(-1.0),
            Value::Double(-1.0),
            Value::String(name.into()),
            Value::SmallInteger(7),
            Value::Double(0.5),
            Value::Null,
        ],
    };
    assert_eq!(ed.insert(&new("new a", 10.0, 151_000.0)).unwrap(), 21);
    assert_eq!(ed.insert(&new("new b", 25.5, 151_100.0)).unwrap(), 22);
    ed.set_geometry(1, Some(square(152_000.0, 449_000.0, 40.0)))
        .unwrap();
    ed.set_value(2, "NAME", Value::String("renamed two".into()))
        .unwrap();
    ed.set_value(5, "NAME", Value::Null).unwrap();
    ed.delete(3).unwrap();
    assert!(ed.delete(4).is_err());
    ed.finish().unwrap();

    let after = rows(&gdb, "parcels");
    assert_eq!(after.len(), 19);
    let get = |oid: i64| after.iter().find(|f| f.oid == oid).unwrap();
    assert_eq!(double(&get(21).attributes[0]), 40.0);
    assert_eq!(double(&get(21).attributes[1]), 100.0);
    assert_eq!(double(&get(22).attributes[0]), 102.0);
    assert_eq!(double(&get(22).attributes[1]), 650.25);
    assert_eq!(double(&get(1).attributes[0]), 160.0);
    assert_eq!(double(&get(1).attributes[1]), 1600.0);
    assert_eq!(get(5).attributes[2], Value::Null);
    assert!(after.iter().all(|f| f.oid != 3));
    for f in &before {
        if ![1, 2, 3, 5].contains(&f.oid) {
            assert_eq!(get(f.oid), f);
        }
    }
    let table = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("parcels")
        .unwrap();
    let hit: Vec<i64> = table
        .features_in_extent(&window(151_990.0, 448_990.0, 152_050.0, 449_050.0))
        .unwrap()
        .iter()
        .map(|f| f.oid)
        .collect();
    assert_eq!(hit, vec![1]);
    let hit: Vec<i64> = table
        .search_attribute(
            "NAME",
            &Value::String("new a".into()),
            &Value::String("renamed two".into()),
        )
        .unwrap()
        .iter()
        .map(|f| f.oid)
        .collect();
    // The range also holds every "parcel N" (between "n" and "r").
    assert!(hit.contains(&21) && hit.contains(&22) && hit.contains(&2));
    let ext = table.geometry_def().unwrap().extent;
    assert!(ext.xmax >= 152_040.0 && ext.ymin <= 449_000.0);
    drop(table);

    // roads: polyline Z with Shape_Length.
    let mut ed = TableEditor::open(base("roads")).unwrap();
    let road = line_z(&[[150_000.0, 449_000.0, 5.0], [150_030.0, 449_040.0, 6.5]]);
    let oid = ed
        .insert(&Feature {
            oid: 0,
            geometry: Some(road.clone()),
            attributes: vec![Value::Null, Value::String("new road".into())],
        })
        .unwrap();
    assert_eq!(oid, 7);
    ed.set_geometry(
        2,
        Some(line_z(&[[0.0, 0.0, 1.0], [0.0, 3.0, 1.0], [4.0, 3.0, 2.0]])),
    )
    .unwrap();
    ed.delete(6).unwrap();
    ed.finish().unwrap();
    let after = rows(&gdb, "roads");
    let get = |oid: i64| after.iter().find(|f| f.oid == oid).unwrap();
    assert_eq!(double(&get(7).attributes[0]), 50.0);
    assert_eq!(double(&get(2).attributes[0]), 7.0);
    assert_eq!(get(7).geometry.as_ref().unwrap().z, road.z);
    assert_eq!(after.len(), 6);

    // pts64: 64-bit OBJECTIDs (version 4 table and v2 spatial index); 3 was deleted.
    let mut ed = TableEditor::open(base("pts64")).unwrap();
    let p = |x: f64, label: &str| Feature {
        oid: 0,
        geometry: Some(Geometry::point(x, 451_000.0)),
        attributes: vec![Value::String(label.into())],
    };
    assert_eq!(ed.insert(&p(151_000.0, "n1")).unwrap(), 9);
    assert_eq!(ed.insert(&p(151_010.0, "n2")).unwrap(), 10);
    ed.update(2, &p(151_020.0, "moved")).unwrap();
    ed.delete(1).unwrap();
    ed.finish().unwrap();
    let after = rows(&gdb, "pts64");
    let oids: Vec<i64> = after.iter().map(|f| f.oid).collect();
    assert_eq!(oids, vec![2, 4, 5, 6, 7, 8, 9, 10]);
    let table = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("pts64")
        .unwrap();
    let hit: Vec<i64> = table
        .features_in_extent(&window(150_990.0, 450_990.0, 151_030.0, 451_010.0))
        .unwrap()
        .iter()
        .map(|f| f.oid)
        .collect();
    assert_eq!(hit, vec![2, 9, 10]);
    drop(table);

    // log: a table with an index on N; delete by predicate, then add and delete fields.
    let gone = delete_rows(
        base("log"),
        |f| matches!(f.attributes[1], Value::Integer(n) if n > 50),
    )
    .unwrap();
    assert_eq!(gone, 4); // 64, 81, 100, 121
    let mut extra = Field::new("EXTRA", FieldType::Integer);
    extra.default = Some(Value::Integer(3));
    add_field(base("log"), &extra).unwrap();
    let mut ed = TableEditor::open(base("log")).unwrap();
    assert_eq!(
        ed.insert(&Feature {
            oid: 0,
            geometry: None,
            attributes: vec![
                Value::String("after".into()),
                Value::Integer(5),
                Value::Null
            ],
        })
        .unwrap(),
        13 // the sequence continues after the deleted 9..12
    );
    ed.finish().unwrap();
    delete_fields(base("parcels"), &["CLASS"]).unwrap();
    assert!(delete_fields(base("parcels"), &["Shape_Area"]).is_err());
    let log = rows(&gdb, "log");
    assert_eq!(log.len(), 9);
    assert!(
        log[..8]
            .iter()
            .all(|f| f.attributes[2] == Value::Integer(3))
    );
    let table = Geodatabase::open(&gdb).unwrap().open_table("log").unwrap();
    let five: Vec<i64> = table
        .search_attribute("N", &Value::Integer(4), &Value::Integer(9))
        .unwrap()
        .iter()
        .map(|f| f.oid)
        .collect();
    assert_eq!(five, vec![3, 4, 13]); // N = 4, 9, 5
    drop(table);
    let parcels = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("parcels")
        .unwrap();
    assert!(parcels.fields().iter().all(|f| f.name != "CLASS"));
    // The rewritten table keeps ArcGIS's index names.
    let ix: Vec<&str> = parcels.indexes().iter().map(|i| i.name.as_str()).collect();
    assert_eq!(ix, ["FDO_OBJECTID", "FDO_Shape", "IX_NAME"]);
    assert_eq!(parcels.row_count(), 19);
    drop(parcels);

    // GDAL reads every edited table as we do (64-bit OBJECTID tables need GDAL 3.9).
    if ogrinfo_available() {
        let mut tables = vec!["parcels", "roads", "log"];
        if gdal_version().is_some_and(|v| v >= (3, 9)) {
            tables.push("pts64");
        }
        for t in tables {
            let d = gdal_diffs(&gdb, t);
            assert!(d.is_empty(), "{t}: {d:#?}");
        }
    }

    // `GF_OUT=/path/x.gdb` keeps this state, to open it in ArcGIS.
    if let Ok(out) = std::env::var("GF_OUT") {
        copy_dir(&gdb, Path::new(&out));
    }

    // Truncate Table restarts the OBJECTIDs.
    truncate_table(base("pts64")).unwrap();
    assert!(rows(&gdb, "pts64").is_empty());
    let mut ed = TableEditor::open(base("pts64")).unwrap();
    assert_eq!(ed.insert(&p(1.0, "first")).unwrap(), 1);
    ed.finish().unwrap();
    std::fs::remove_dir_all(&dir).ok();
}

/// `GF_GDB=/path/x.gdb cargo test -p geoforge-filegdb --test edit_arcgis -- --ignored`:
/// compares every table of a geodatabase (e.g. one ArcGIS edited after us) with GDAL.
#[test]
#[ignore]
fn matches_gdal_at() {
    let gdb = PathBuf::from(std::env::var("GF_GDB").expect("GF_GDB"));
    let names: Vec<String> = Geodatabase::open(&gdb)
        .unwrap()
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| !n.starts_with("GDB_"))
        .collect();
    for n in &names {
        let d = gdal_diffs(&gdb, n);
        assert!(d.is_empty(), "{n}: {d:#?}");
        eprintln!("{n}: {} rows equal to GDAL's", rows(&gdb, n).len());
    }
}

/// Curved geometry (ArcGIS buffers are stored with circular arcs): the measures we compute
/// for a shape inserted into its own class equal the ones ArcGIS stored for it.
#[test]
fn measures_of_curves_equal_arcgis() {
    let dir = std::env::temp_dir().join(format!("gf-edit-curves-{}", std::process::id()));
    let gdb = dir.join("b.gdb");
    copy_dir(&root().join("testdata/gdb-reference/buffers.gdb"), &gdb);
    let g = Geodatabase::open(&gdb).unwrap();
    let names: Vec<String> = g
        .entries()
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| !n.starts_with("GDB_"))
        .collect();
    let mut checked = 0;
    for name in names {
        let t = g.open_table(&name).unwrap();
        let (Some(li), Some(ai)) = (
            t.fields().iter().position(|f| f.name == "Shape_Length"),
            t.fields().iter().position(|f| f.name == "Shape_Area"),
        ) else {
            continue;
        };
        let rows: Vec<Feature> = t.features().take(5).map(|f| f.unwrap()).collect();
        drop(t);
        let mut ed = TableEditor::open(g.table_base(&name).unwrap()).unwrap();
        for f in rows {
            let mut copy = f.clone();
            copy.attributes[li] = Value::Null;
            copy.attributes[ai] = Value::Null;
            let oid = ed.insert(&copy).unwrap();
            let got = ed.get(oid).unwrap().unwrap();
            for i in [li, ai] {
                let (want, have) = (double(&f.attributes[i]), double(&got.attributes[i]));
                assert!(
                    (want - have).abs() <= 1e-7 * want.max(1.0),
                    "{name} {}: ArcGIS {want}, ours {have}",
                    f.oid
                );
            }
            checked += 1;
        }
        ed.finish().unwrap();
    }
    assert!(checked >= 15, "{checked}");
    std::fs::remove_dir_all(&dir).ok();
}
