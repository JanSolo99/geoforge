//! Subtypes and field defaults written by GeoForge read back as ArcGIS Pro's do
//! (testdata/gdb-reference/subtypes.gdb: table `base` with subtype field `kind`, subtypes Road (1) and
//! Rail (2, default), per-subtype domains and defaults on `lanes` and `name`, and field defaults).

use std::path::{Path, PathBuf};

use geoforge_core::{Feature, Field, FieldType, Value};
use geoforge_filegdb::schema::{
    Domain, DomainKind, MergePolicy, SplitPolicy, Subtype, SubtypeField, Subtypes,
};
use geoforge_filegdb::{Geodatabase, GeodatabaseWriter};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn domains() -> [Domain; 2] {
    [
        Domain {
            name: "lanes_dom".into(),
            description: "lanes".into(),
            field_type: FieldType::Integer,
            split_policy: SplitPolicy::DefaultValue,
            merge_policy: MergePolicy::DefaultValue,
            kind: DomainKind::Range {
                min: Value::Integer(1),
                max: Value::Integer(8),
            },
        },
        Domain {
            name: "names".into(),
            description: "names".into(),
            field_type: FieldType::String,
            split_policy: SplitPolicy::DefaultValue,
            merge_policy: MergePolicy::DefaultValue,
            kind: DomainKind::Coded(vec![(Value::String("a".into()), "A".into())]),
        },
    ]
}

fn fields() -> Vec<Field> {
    let mut kind = Field::new("kind", FieldType::SmallInteger);
    kind.nullable = true;
    let mut name = Field::new("name", FieldType::String);
    name.length = 20;
    name.default = Some(Value::String("x".into()));
    let mut grade = Field::new("grade", FieldType::Double);
    grade.default = Some(Value::Double(1.5));
    vec![kind, name, Field::new("lanes", FieldType::Integer), grade]
}

fn subtypes() -> Subtypes {
    let sf = |field: &str, domain: Option<&str>, default: Option<Value>| SubtypeField {
        field: field.into(),
        domain: domain.map(Into::into),
        default,
    };
    Subtypes {
        field: "kind".into(),
        default_code: 2,
        subtypes: vec![
            Subtype {
                code: 1,
                name: "Road".into(),
                fields: vec![
                    sf("lanes", Some("lanes_dom"), Some(Value::Integer(2))),
                    sf("name", Some("names"), None),
                ],
            },
            Subtype {
                code: 2,
                name: "Rail".into(),
                fields: vec![sf("name", Some("names"), None)],
            },
        ],
    }
}

fn build(gdb: &Path) {
    let mut w = GeodatabaseWriter::create(gdb).unwrap();
    for d in domains() {
        w.add_domain(&d).unwrap();
    }
    let mut t = w
        .create_table_with_subtypes("base", None, false, false, &fields(), None, subtypes())
        .unwrap();
    t.write(&Feature {
        oid: 0,
        geometry: None,
        attributes: vec![
            Value::SmallInteger(1),
            Value::String("a".into()),
            Value::Integer(3),
            Value::Double(0.5),
        ],
    })
    .unwrap();
    w.finish_table(t).unwrap();
    w.finish().unwrap();
}

#[test]
fn subtypes_and_defaults_read_back_as_arcgis_has_them() {
    let ours = std::env::temp_dir().join(format!("gf-subtypes-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&ours).ok();
    build(&ours);
    let (a, b) = (
        Geodatabase::open(&ours).unwrap(),
        Geodatabase::open(root().join("testdata/gdb-reference/subtypes.gdb")).unwrap(),
    );
    let (ta, tb) = (a.open_table("base").unwrap(), b.open_table("base").unwrap());
    assert_eq!(ta.subtypes(), tb.subtypes());
    assert!(ta.subtypes().is_some());
    // Defaults: the subtype field takes the default subtype's code; text and floating point ones too.
    let defaults = |t: &geoforge_filegdb::Table| {
        t.fields()
            .iter()
            .map(|f| (f.name.clone(), f.default.clone()))
            .collect::<Vec<_>>()
    };
    let want: Vec<_> = defaults(&tb)
        .into_iter()
        .filter(|(n, _)| ["kind", "name", "grade"].contains(&n.as_str()))
        .collect();
    let got: Vec<_> = defaults(&ta)
        .into_iter()
        .filter(|(n, _)| ["kind", "name", "grade"].contains(&n.as_str()))
        .collect();
    assert_eq!(got, want);
    assert_eq!(
        got.iter().find(|(n, _)| n == "kind").unwrap().1,
        Some(Value::SmallInteger(2))
    );
    // Domains used only by subtypes still belong to the table, and the data is there.
    assert_eq!(a.domains().len(), 2);
    assert_eq!(ta.row_count(), 1);
    // The field-level domain is not set by a subtype's domain.
    assert!(ta.all_fields().iter().all(|f| f.domain.is_none()));

    if geoforge_testkit::ogrinfo_available() {
        let out = std::process::Command::new("python3")
            .arg(root().join("scripts/gdb-snapshot.py"))
            .arg(&ours)
            .output()
            .unwrap();
        let ours_snap: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let theirs: serde_json::Value = serde_json::from_slice(
            &std::process::Command::new("python3")
                .arg(root().join("scripts/gdb-snapshot.py"))
                .arg(root().join("testdata/gdb-reference/subtypes.gdb"))
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        // The relationships tie the table to both domains, as in ArcGIS.
        let rels = |s: &serde_json::Value| {
            s["rels"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r.as_str().unwrap().contains("17E08ADB"))
                .count()
        };
        assert_eq!((rels(&ours_snap), rels(&theirs)), (2, 2));
        // The Subtypes element of the definition is ArcGIS's, character for character (whitespace aside).
        let block = |s: &serde_json::Value| {
            let d = s["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i["name"] == "base")
                .unwrap()["definition"]
                .as_str()
                .unwrap()
                .to_string();
            let from = d.find("<SubtypeFieldName>").unwrap();
            let to = d.find("</Subtypes>").unwrap();
            d[from..to].replace("\"", "'")
        };
        assert_eq!(block(&ours_snap), block(&theirs));
    }
    std::fs::remove_dir_all(&ours).ok();
}

#[test]
fn bad_subtype_definitions_are_refused() {
    let dir = std::env::temp_dir().join(format!("gf-subtypes-bad-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut w = GeodatabaseWriter::create(&dir).unwrap();
    let mut s = subtypes();
    s.field = "name".into(); // text: not a subtype field
    assert!(
        w.create_table_with_subtypes("t", None, false, false, &fields(), None, s)
            .is_err()
    );
    let mut s = subtypes();
    s.default_code = 9;
    assert!(
        w.create_table_with_subtypes("t", None, false, false, &fields(), None, s)
            .is_err()
    );
    // A domain that does not exist.
    assert!(
        w.create_table_with_subtypes("t", None, false, false, &fields(), None, subtypes())
            .is_err()
    );
    let mut s = subtypes();
    s.subtypes[1].code = 1;
    assert!(
        w.create_table_with_subtypes("t", None, false, false, &fields(), None, s)
            .is_err()
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// `GF_OUT=/path/x.gdb cargo test -p geoforge-filegdb --test subtypes -- --ignored`: keeps the
/// geodatabase for opening in ArcGIS.
#[test]
#[ignore]
fn write_for_arcgis() {
    let out = std::env::var("GF_OUT").expect("GF_OUT");
    std::fs::remove_dir_all(&out).ok();
    build(Path::new(&out));
}
