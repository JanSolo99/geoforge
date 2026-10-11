//! GeoPackage writing round trips: every field type ArcGIS can store, Z/M
//! shapes, null and empty geometries, adding a second table and replacing one;
//! the files must pass SQLite's integrity and R*Tree checks and read back the
//! same through the GeoPackage reader.

use std::path::PathBuf;
use std::process::Command;

use chrono::{NaiveDate, NaiveDateTime};
use geoforge_core::{Feature, FeatureClass, Field, FieldType, Geometry, GeometryKind, Value};
use geoforge_gpkg::GeoPackage;
use geoforge_gpkg::write::{NewTable, SpatialRef, write_table};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("geoforge-gpkg-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn utm() -> SpatialRef {
    SpatialRef {
        srs_id: 32633,
        srs_name: "WGS_1984_UTM_Zone_33N".into(),
        organization: "EPSG".into(),
        organization_coordsys_id: 32633,
        definition: "PROJCS[\"WGS_1984_UTM_Zone_33N\"]".into(),
        description: Some("WGS 1984 UTM Zone 33N".into()),
        definition_12_063: "undefined".into(),
    }
}

fn field(name: &str, t: FieldType, length: u32) -> Field {
    let mut f = Field::new(name, t);
    f.length = length;
    f
}

fn polygon_zm(x: f64) -> Geometry {
    // Clockwise outer ring with a counterclockwise hole (Esri order).
    let xy = vec![
        [x, 0.0],
        [x, 10.0],
        [x + 10.0, 10.0],
        [x + 10.0, 0.0],
        [x, 0.0],
        [x + 2.0, 2.0],
        [x + 4.0, 2.0],
        [x + 4.0, 4.0],
        [x + 2.0, 4.0],
        [x + 2.0, 2.0],
    ];
    Geometry {
        kind: GeometryKind::Polygon,
        parts: vec![0, 5],
        part_types: None,
        z: Some((0..10).map(|i| i as f64).collect()),
        m: Some((0..10).map(|i| i as f64 * 0.5).collect()),
        xy,
        curves: Vec::new(),
    }
}

fn sqlite_ok(path: &PathBuf, rtrees: &[&str]) {
    if Command::new("sqlite3").arg("-version").output().is_err() {
        return;
    }
    let checks: Vec<String> = rtrees
        .iter()
        .map(|r| format!("rtreecheck('{r}')"))
        .collect();
    let sql = format!("PRAGMA integrity_check; SELECT {};", checks.join(", "));
    let out = Command::new("sqlite3").arg(path).arg(sql).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.trim(),
        format!("ok\n{}", vec!["ok"; rtrees.len()].join("|")),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn writes_and_reads_back() {
    let path = temp("rt.gpkg");
    std::fs::remove_file(&path).ok();
    let fields = vec![
        field("f_short", FieldType::SmallInteger, 2),
        field("f_long", FieldType::Integer, 4),
        field("f_big", FieldType::BigInteger, 8),
        field("f_float", FieldType::Single, 4),
        field("f_double", FieldType::Double, 8),
        field("f_text", FieldType::String, 30),
        field("f_date", FieldType::Date, 8),
        field("f_donly", FieldType::DateOnly, 8),
        field("f_guid", FieldType::Guid, 38),
        field("f_blob", FieldType::Blob, 0),
    ];
    let date =
        NaiveDateTime::parse_from_str("2024-01-02 03:04:05.678", "%Y-%m-%d %H:%M:%S%.f").unwrap();
    let full = vec![
        Value::SmallInteger(-7),
        Value::Integer(70000),
        Value::BigInteger(1 << 40),
        Value::Single(1.5),
        Value::Double(2.0),
        Value::String("text é".into()),
        Value::Date(date),
        Value::DateOnly(NaiveDate::from_ymd_opt(2024, 1, 2).unwrap()),
        Value::Guid("{12345678-1234-1234-1234-123456789ABC}".into()),
        Value::Blob(vec![0, 1, 2]),
    ];
    let features: Vec<Feature> = (0..300)
        .map(|k| Feature {
            oid: 1000 + k,
            geometry: match k % 10 {
                3 => None,
                _ => Some(polygon_zm(k as f64 * 20.0)),
            },
            attributes: if k % 7 == 0 {
                vec![Value::Null; fields.len()]
            } else {
                full.clone()
            },
        })
        .collect();
    let sr = utm();
    write_table(
        &path,
        &NewTable {
            name: "polys",
            geometry: Some((GeometryKind::Polygon, true, true)),
            fields: &fields,
            spatial_ref: Some(&sr),
            xy_domain: Some((-5120900.0, -9998100.0, 0.0001)),
            features: &features,
        },
    )
    .unwrap();
    // A second table, then the first replaced by points (one empty).
    let point = |x: f64| Some(Geometry::point(x, 5.0));
    let mut empty = Geometry::point(0.0, 0.0);
    empty.xy.clear();
    let points: Vec<Feature> = [point(1.0), Some(empty), point(3.0)]
        .into_iter()
        .map(|g| Feature {
            oid: 0,
            geometry: g,
            attributes: vec![Value::Integer(1)],
        })
        .collect();
    let pfields = vec![field("n", FieldType::Integer, 4)];
    for name in ["pts", "polys2"] {
        write_table(
            &path,
            &NewTable {
                name,
                geometry: Some((GeometryKind::Point, false, false)),
                fields: &pfields,
                spatial_ref: Some(&sr),
                xy_domain: None,
                features: &points,
            },
        )
        .unwrap();
    }
    sqlite_ok(
        &path,
        &["rtree_polys_Shape", "rtree_pts_Shape", "rtree_polys2_Shape"],
    );

    let gpkg = GeoPackage::open(&path).unwrap();
    let names: Vec<&str> = gpkg
        .contents()
        .iter()
        .map(|c| c.table_name.as_str())
        .collect();
    assert_eq!(names, ["polys", "pts", "polys2"]);
    let polys = gpkg.open_table("main.polys").unwrap();
    assert_eq!(polys.oid_field_name(), "OBJECTID");
    assert!(polys.has_z() && polys.has_m());
    let read: Vec<Feature> = polys.features().map(Result::unwrap).collect();
    assert_eq!(read.len(), 300);
    for (k, (f, src)) in read.iter().zip(&features).enumerate() {
        assert_eq!(f.oid, k as i64 + 1, "ObjectIDs renumbered from 1");
        assert_eq!(f.geometry, src.geometry, "feature {k}");
        for (a, b) in f.attributes.iter().zip(&src.attributes) {
            let expected = match b {
                // Dates are stored rounded to whole seconds.
                Value::Date(d) => Value::Date(
                    NaiveDateTime::parse_from_str("2024-01-02 03:04:06", "%Y-%m-%d %H:%M:%S")
                        .unwrap_or(*d),
                ),
                Value::Guid(s) => Value::String(s.clone()),
                v => v.clone(),
            };
            assert_eq!(*a, expected, "feature {k}");
        }
    }
    let pts = gpkg.open_table("polys2").unwrap();
    let read: Vec<Feature> = pts.features().map(Result::unwrap).collect();
    assert_eq!(
        read[1].geometry.as_ref().map(|g| g.xy.len()),
        Some(0),
        "empty point"
    );

    // Replacing a table keeps one copy of it and its index.
    write_table(
        &path,
        &NewTable {
            name: "pts",
            geometry: Some((GeometryKind::Point, false, false)),
            fields: &pfields,
            spatial_ref: Some(&sr),
            xy_domain: None,
            features: &points[..1],
        },
    )
    .unwrap();
    sqlite_ok(
        &path,
        &["rtree_polys_Shape", "rtree_pts_Shape", "rtree_polys2_Shape"],
    );
    let gpkg = GeoPackage::open(&path).unwrap();
    assert_eq!(gpkg.contents().len(), 3);
    assert_eq!(gpkg.open_table("pts").unwrap().features().count(), 1);
    std::fs::remove_file(&path).ok();
}
