//! XML workspace imports compared with saved ArcGIS Pro 3.7 controls.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use unigis_core::{Feature, GeometryKind, Value};
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/geoproc/xml_workspace")
        .join(name)
}
fn root(label: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("ug-import-xml-{}-{label}", std::process::id()));
    std::fs::create_dir(&p).unwrap();
    p
}
fn target(dir: &Path) -> PathBuf {
    let p = dir.join("target.gdb");
    let mut w = GeodatabaseWriter::create(&p).unwrap();
    let mut t = w
        .create_table("sentinel", None, false, false, &[], None)
        .unwrap();
    t.write(&Feature {
        oid: 0,
        geometry: None,
        attributes: vec![],
    })
    .unwrap();
    w.finish_table(t).unwrap();
    w.finish().unwrap();
    p
}
fn run(target: &Path, file: &Path, params: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(["run", "import_xml_workspace_document"])
        .arg(format!("target_geodatabase={}", target.display()))
        .arg(format!("in_file={}", file.display()))
        .args(params)
        .output()
        .unwrap()
}
fn success(out: Output) {
    assert!(out.status.success(), "{out:?}");
}
fn rows(db: &Geodatabase, name: &str) -> Vec<Feature> {
    db.open_table(name)
        .unwrap()
        .features()
        .map(Result::unwrap)
        .collect()
}
fn bytes(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .filter(|(n, _)| !n.ends_with(".lock"))
        .collect()
}
#[test]
fn both_encodings_import_values_geometry_and_original_objectids() {
    for encoding in ["binary", "normalized"] {
        let dir = root(encoding);
        let out = target(&dir);
        let xml = fixture(&format!("{encoding}.xml"));
        let before = std::fs::read(&xml).unwrap();
        success(run(&out, &xml, &[]));
        success(run(
            &out,
            &fixture(&format!("extended_{encoding}.xml")),
            &["config_keyword=DEFAULTS"],
        ));
        let db = Geodatabase::open(&out).unwrap();
        assert_eq!(db.entries().len(), 7);
        assert_eq!(rows(&db, "sentinel").len(), 1);
        let labels = rows(&db, "labels");
        assert_eq!(
            labels[0].attributes,
            vec![Value::Integer(4), Value::String("A & B <town>".into())]
        );
        assert_eq!(labels[1].attributes, vec![Value::Null, Value::Null]);
        let table = db.open_table("labels").unwrap();
        assert_eq!(table.fields()[0].alias.as_deref(), Some("Count"));
        assert_eq!(table.fields()[1].length, 40);
        let points = rows(&db, "points");
        let p = points[0].geometry.as_ref().unwrap();
        assert_eq!(p.kind, GeometryKind::Point);
        assert_eq!(p.xy, [[1.0, 2.0]]);
        assert_eq!(points[0].attributes, [Value::SmallInteger(3)]);
        let table = db.open_table("points").unwrap();
        let gd = table.geometry_def().unwrap();
        assert_eq!(
            (gd.x_origin, gd.y_origin, gd.xy_scale, gd.xy_tolerance),
            (-5120900.0, -9998100.0, 10000.0, 0.001)
        );
        assert!(gd.srs_wkt.as_ref().unwrap().contains("32632"));
        let types = rows(&db, "types");
        assert_eq!(types.iter().map(|r| r.oid).collect::<Vec<_>>(), [1, 3]);
        let stamp = chrono::NaiveDate::from_ymd_opt(2024, 3, 4)
            .unwrap()
            .and_hms_opt(5, 6, 7)
            .unwrap();
        assert_eq!(
            types[0].attributes,
            vec![
                Value::SmallInteger(-5),
                Value::Integer(100),
                Value::Single(1.5),
                Value::Double(-2.125),
                Value::Date(stamp),
                Value::Guid("{12345678-1234-5678-9ABC-DEF012345678}".into()),
                Value::String("café 空".into())
            ]
        );
        assert_eq!(
            types[1].attributes[4..],
            [Value::Null, Value::Null, Value::String("third".into())]
        );
        assert_eq!(
            db.open_table("types").unwrap().fields()[1].default,
            Some(Value::Integer(42))
        );
        let lines = rows(&db, "lines");
        let g = lines[0].geometry.as_ref().unwrap();
        assert_eq!(g.kind, GeometryKind::Polyline);
        assert_eq!(g.parts, [0, 2]);
        assert_eq!(g.xy, [[0.0, 0.0], [4.0, 3.0], [5.0, 6.0], [6.0, 7.0]]);
        assert_eq!(lines[0].attributes, [Value::Double(6.414213562373095)]);
        let areas = rows(&db, "areas");
        let g = areas[0].geometry.as_ref().unwrap();
        assert_eq!(g.kind, GeometryKind::Polygon);
        assert_eq!(
            g.xy,
            [[0.0, 0.0], [0.0, 4.0], [4.0, 4.0], [4.0, 0.0], [0.0, 0.0]]
        );
        assert_eq!(
            areas[0].attributes,
            [Value::Double(16.0), Value::Double(16.0)]
        );
        let many = rows(&db, "many");
        assert_eq!(
            many[0].geometry.as_ref().unwrap().kind,
            GeometryKind::Multipoint
        );
        assert_eq!(
            many[0].geometry.as_ref().unwrap().xy,
            [[1.0, 2.0], [3.0, 4.0]]
        );
        assert_eq!(std::fs::read(xml).unwrap(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
#[test]
fn schema_only_and_schema_document_default_have_fields_without_rows() {
    for (label, file, args) in [
        (
            "schema",
            "extended_binary.xml",
            vec!["import_type=SCHEMA_ONLY"],
        ),
        ("schema-default", "schema.xml", vec![]),
    ] {
        let dir = root(label);
        let out = target(&dir);
        success(run(&out, &fixture(file), &args));
        let db = Geodatabase::open(&out).unwrap();
        for name in ["types", "lines", "areas", "many"] {
            assert!(rows(&db, name).is_empty());
        }
        assert_eq!(
            db.open_table("types").unwrap().fields()[1].default,
            Some(Value::Integer(42))
        );
        assert_eq!(
            db.open_table("areas").unwrap().geometry_kind(),
            Some(GeometryKind::Polygon)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
#[test]
fn collisions_get_unique_names_and_leave_existing_rows_intact() {
    let dir = root("collision");
    let out = target(&dir);
    let xml = fixture("binary.xml");
    success(run(&out, &xml, &[]));
    let db = Geodatabase::open(&out).unwrap();
    let original_rows = rows(&db, "labels");
    let table_base = db.table_base("labels").unwrap();
    let before = bytes(&out);
    drop(db);
    for _ in 0..2 {
        success(run(&out, &xml, &[]));
    }
    let db = Geodatabase::open(&out).unwrap();
    for name in ["labels", "labels_1", "labels_2"] {
        assert_eq!(rows(&db, name), original_rows);
    }
    assert_eq!(
        rows(&db, "points_2")[0].geometry.as_ref().unwrap().xy,
        [[1.0, 2.0]]
    );
    for (name, data) in before {
        if name.starts_with(table_base.file_stem().unwrap().to_str().unwrap()) {
            assert_eq!(std::fs::read(out.join(name)).unwrap(), data);
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn failure_preflight_and_mid_write_failure_preserve_target_and_source() {
    let dir = root("failures");
    let out = target(&dir);
    let original = bytes(&out);
    let good = std::fs::read_to_string(fixture("binary.xml")).unwrap();
    let file = dir.join("bad.xml");
    let bad = [
        ("not XML".to_owned(), "invalid XML"),
        (
            good.replace(
                "<Domains xsi:type='esri:ArrayOfDomain'></Domains>",
                "<Domains><Domain/></Domains>",
            ),
            "domains",
        ),
        (
            good.replace(
                "<DatasetType>esriDTFeatureClass</DatasetType>",
                "<DatasetType>esriDTRasterDataset</DatasetType>",
            ),
            "standalone",
        ),
        (
            good.replace("AQAAAAAAAAAAAPA/AAAAAAAAAEA=", "AQAA"),
            "truncated",
        ),
        (
            good.replace(
                "<Value xsi:type='xs:short'>3</Value>",
                "<Value xsi:type='xs:short'>999999</Value>",
            ),
            "number",
        ),
    ];
    for (xml, _) in &bad {
        assert_ne!(*xml, good);
    }
    for (xml, expected) in bad {
        std::fs::write(&file, &xml).unwrap();
        let result = run(&out, &file, &[]);
        assert!(!result.status.success(), "{expected}: {result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{expected}: {result:?}"
        );
        assert_eq!(bytes(&out), original);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), xml);
    }
    // Three feature classes are written first; a too-long string in the last table
    // fails inside the writer, exercising rollback of tables already finished.
    let xml = std::fs::read_to_string(fixture("extended_binary.xml"))
        .unwrap()
        .replace("café 空", &"x".repeat(51));
    std::fs::write(&file, &xml).unwrap();
    let result = run(&out, &file, &[]);
    assert!(!result.status.success(), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("text longer"),
        "{result:?}"
    );
    assert_eq!(bytes(&out), original);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), xml);
    for params in [
        vec!["import_type=BAD"],
        vec!["config_keyword=MAX_FILE_SIZE_256TB"],
    ] {
        assert!(!run(&out, &fixture("binary.xml"), &params).status.success());
        assert_eq!(bytes(&out), original);
    }
    assert!(
        !run(&dir.join("absent.gdb"), &fixture("binary.xml"), &[])
            .status
            .success()
    );
    assert!(!dir.join("absent.gdb").exists());
    std::fs::remove_dir_all(dir).unwrap();
}
