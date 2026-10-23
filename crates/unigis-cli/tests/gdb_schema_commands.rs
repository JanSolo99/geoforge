//! `ug` commands that edit a geodatabase's schema: subtypes, domains and defaults of fields, and
//! relationship classes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use unigis_filegdb::Geodatabase;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn gf(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ug"))
        .args(args)
        .output()
        .unwrap()
}

fn copy_of(reference: &str, tag: &str) -> PathBuf {
    let to = std::env::temp_dir().join(format!("gf-schema-{tag}-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&to).ok();
    std::fs::create_dir_all(&to).unwrap();
    for e in std::fs::read_dir(root().join(reference)).unwrap().flatten() {
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
    to
}

fn ok(out: Output) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn subtype_commands() {
    let gdb = copy_of("testdata/gdb-reference/subtypes.gdb", "sub");
    let table = gdb.join("base");
    let t = table.to_str().unwrap();
    ok(gf(&["remove-subtype", t, "1"]));
    ok(gf(&["add-subtype", t, "3", "Air"]));
    ok(gf(&["set-default-subtype", t, "3"]));
    ok(gf(&["assign-domain-to-field", t, "lanes", "lanes_dom"]));
    ok(gf(&["assign-default-to-field", t, "lanes", "4"]));
    ok(gf(&[
        "assign-default-to-field",
        t,
        "lanes",
        "5",
        "--subtype-code",
        "3",
    ]));
    let db = Geodatabase::open(&gdb).unwrap();
    let base = db.open_table("base").unwrap();
    let st = base.subtypes().unwrap();
    assert_eq!(
        (
            st.default_code,
            st.subtypes.iter().map(|s| s.code).collect::<Vec<_>>()
        ),
        (3, vec![2, 3])
    );
    let lanes = base.fields().iter().find(|f| f.name == "lanes").unwrap();
    assert_eq!(lanes.domain.as_deref(), Some("lanes_dom"));
    assert_eq!(lanes.default, Some(unigis_core::Value::Integer(4)));
    // A default the domain refuses, and a subtype that is not there.
    let bad = gf(&["assign-default-to-field", t, "lanes", "99"]);
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("160080"));
    let bad = gf(&["remove-subtype", t, "9"]);
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("000192"));
    ok(gf(&["assign-default-to-field", t, "lanes"]));
    ok(gf(&["remove-domain-from-field", t, "lanes"]));
    std::fs::remove_dir_all(&gdb).ok();
}

#[test]
fn relationship_class_command() {
    let gdb = copy_of("testdata/gdb-reference/relationship_tables.gdb", "rel");
    let g = gdb.to_str().unwrap();
    let item = |n: &str| format!("{g}/{n}");
    ok(gf(&[
        "create-relationship-class",
        &item("Parcels"),
        &item("Owners"),
        &item("plain"),
        "COMPOSITE",
        "has",
        "of",
        "FORWARD",
        "ONE_TO_ONE",
        "NONE",
        "Pk_Id",
        "owner_id",
    ]));
    ok(gf(&[
        "create-relationship-class",
        &item("Parcels"),
        &item("Owners"),
        &item("pairs"),
        "SIMPLE",
        "x",
        "y",
        "NONE",
        "MANY_TO_MANY",
        "ATTRIBUTED",
        "Pk_Id",
        "pk",
        "owner_id",
        "ok",
    ]));
    let db = Geodatabase::open(&gdb).unwrap();
    let names: Vec<_> = db
        .relationship_classes()
        .iter()
        .map(|r| r.name.as_str())
        .collect();
    assert_eq!(names, ["att4", "mn", "pairs", "plain"]);
    assert_eq!(db.open_table("pairs").unwrap().all_fields().len(), 3);
    // An attributed class needs its four keys; a bad cardinality is refused.
    assert!(
        !gf(&[
            "create-relationship-class",
            &item("Parcels"),
            &item("Owners"),
            &item("bad"),
            "SIMPLE",
            "x",
            "y",
            "NONE",
            "MANY_TO_MANY",
            "ATTRIBUTED",
            "Pk_Id",
            "pk"
        ])
        .status
        .success()
    );
    assert!(
        !gf(&[
            "create-relationship-class",
            &item("Parcels"),
            &item("Owners"),
            &item("bad2"),
            "SIMPLE",
            "x",
            "y",
            "NONE",
            "SOME_TO_SOME",
            "NONE",
            "Pk_Id",
            "owner_id"
        ])
        .status
        .success()
    );
    std::fs::remove_dir_all(&gdb).ok();
}
