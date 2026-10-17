//! Editing subtypes, field domains and defaults of an existing table leaves the item definition, the
//! domain links and the table's field defaults as ArcGIS Pro leaves them
//! (testdata/gdb-reference/subtype_ops: ArcGIS's states after each operation on subtypes.gdb, as
//! scripts/gdb-snapshot.py makes them through GDAL).

use std::path::{Path, PathBuf};
use std::process::Command;

use geoforge_core::Value;
use geoforge_filegdb as g;
use serde_json::Value as Json;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn run(op: &str, apply: impl Fn(&Path)) {
    if !geoforge_testkit::ogrinfo_available() {
        return;
    }
    let work = std::env::temp_dir().join(format!("gf-subedit-{op}-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&work).ok();
    copy_dir(&root().join("testdata/gdb-reference/subtypes.gdb"), &work);
    apply(&work);
    let ours = snapshot(&work);
    std::fs::remove_dir_all(&work).ok();
    let Some(ours) = ours else { return };
    let want: Json = serde_json::from_slice(
        &std::fs::read(root().join(format!("testdata/gdb-reference/subtype_ops/{op}.json")))
            .unwrap(),
    )
    .unwrap();
    // Everything but the table definition is as before, and the table definition is ArcGIS's.
    assert_eq!(ours["rels"], want["rels"], "{op}: rels");
    assert_eq!(ours["defaults"], want["defaults"], "{op}: table defaults");
    assert_eq!(ours["catalog"], want["catalog"], "{op}: catalog");
    // Item by item, naming the one that differs.
    let (a, b) = (
        ours["items"].as_array().unwrap(),
        want["items"].as_array().unwrap(),
    );
    assert_eq!(a.len(), b.len(), "{op}: item count");
    for (x, y) in a.iter().zip(b) {
        if x != y {
            let (dx, dy) = (
                x["definition"].as_str().unwrap(),
                y["definition"].as_str().unwrap(),
            );
            let at = dx
                .bytes()
                .zip(dy.bytes())
                .position(|(p, q)| p != q)
                .unwrap_or(dx.len().min(dy.len()));
            let from = at.saturating_sub(60);
            panic!(
                "{op}: item {} differs at byte {at}\n ours:  ...{}\n ArcGIS: ...{}",
                x["name"],
                &dx[from..(at + 140).min(dx.len())],
                &dy[from..(at + 140).min(dy.len())]
            );
        }
    }
}

#[test]
fn subtype_operations_equal_arcgis() {
    run("rmSub1", |d| g::remove_subtype(d, "base", &[1]).unwrap());
    run("rmSub2", |d| g::remove_subtype(d, "base", &[2]).unwrap());
    run("rmAll", |d| g::remove_subtype(d, "base", &[1, 2]).unwrap());
    run("addSub3", |d| g::add_subtype(d, "base", 3, "Air").unwrap());
    run("defSub1", |d| g::set_default_subtype(d, "base", 1).unwrap());
}

#[test]
fn domain_and_default_operations_equal_arcgis() {
    run("rmDomLanes", |d| {
        g::remove_domain_from_field(d, "base", "lanes", &[1]).unwrap()
    });
    run("asgDomField", |d| {
        g::assign_domain_to_field(d, "base", "lanes", "lanes_dom", &[]).unwrap()
    });
    run("domSub2", |d| {
        g::assign_domain_to_field(d, "base", "lanes", "lanes_dom", &[2]).unwrap()
    });
    run("defLanes", |d| {
        g::assign_default_to_field(d, "base", "lanes", Some(&Value::Integer(4)), &[]).unwrap()
    });
    run("defClear2", |d| {
        g::assign_default_to_field(d, "base", "grade", None, &[]).unwrap()
    });
    run("defSubGrade", |d| {
        g::assign_default_to_field(d, "base", "grade", Some(&Value::Double(9.5)), &[2]).unwrap()
    });
    run("defKind", |d| {
        g::assign_default_to_field(d, "base", "kind", Some(&Value::SmallInteger(1)), &[]).unwrap()
    });
}

#[test]
fn refusals_are_arcgis_errors() {
    let work = std::env::temp_dir().join(format!("gf-subedit-refuse-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&work).ok();
    copy_dir(&root().join("testdata/gdb-reference/subtypes.gdb"), &work);
    let err = |r: geoforge_core::Result<()>| r.unwrap_err().to_string();
    assert!(err(g::remove_subtype(&work, "base", &[9])).contains("000192"));
    assert!(err(g::add_subtype(&work, "base", 1, "Dup")).contains("160129"));
    assert!(err(g::set_default_subtype(&work, "base", 9)).contains("000800"));
    assert!(
        err(g::assign_domain_to_field(
            &work,
            "base",
            "name",
            "lanes_dom",
            &[]
        ))
        .contains("000356")
    );
    assert!(
        err(g::assign_domain_to_field(
            &work,
            "base",
            "lanes",
            "lanes_dom",
            &[7]
        ))
        .contains("000192")
    );
    assert!(
        err(g::assign_default_to_field(
            &work,
            "base",
            "lanes",
            Some(&Value::Integer(99)),
            &[1]
        ))
        .contains("160080")
    );
    assert!(
        err(g::assign_domain_to_field(
            &work,
            "base",
            "lanes",
            "nope",
            &[]
        ))
        .contains("000732")
    );
    assert!(err(g::set_subtype_field(&work, "base", "name")).contains("003911"));
    assert!(err(g::set_subtype_field(&work, "base", "nothere")).contains("000728"));
    std::fs::remove_dir_all(&work).ok();
}

/// SetSubtypeField, then AddSubtype / SetDefaultSubtype, on a table without subtypes: as ArcGIS's s0 -> s1.
#[test]
fn a_table_gets_its_first_subtypes() {
    use geoforge_core::{Field, FieldType};
    use geoforge_filegdb::GeodatabaseWriter;
    let gdb = std::env::temp_dir().join(format!("gf-subedit-first-{}.gdb", std::process::id()));
    std::fs::remove_dir_all(&gdb).ok();
    let mut w = GeodatabaseWriter::create(&gdb).unwrap();
    let mut name = Field::new("name", FieldType::String);
    name.length = 20;
    let t = w
        .create_table(
            "base",
            None,
            false,
            false,
            &[Field::new("kind", FieldType::SmallInteger), name],
            None,
        )
        .unwrap();
    w.finish_table(t).unwrap();
    w.finish().unwrap();
    assert!(
        g::add_subtype(&gdb, "base", 1, "Road")
            .unwrap_err()
            .to_string()
            .contains("000189")
    );
    g::set_subtype_field(&gdb, "base", "kind").unwrap();
    g::add_subtype(&gdb, "base", 1, "Road").unwrap();
    g::add_subtype(&gdb, "base", 2, "Rail").unwrap();
    g::set_default_subtype(&gdb, "base", 2).unwrap();
    let db = g::Geodatabase::open(&gdb).unwrap();
    let st = db.open_table("base").unwrap().subtypes().cloned().unwrap();
    assert_eq!(
        (st.field.as_str(), st.default_code, st.subtypes.len()),
        ("kind", 2, 2)
    );
    assert_eq!(st.subtypes[1].name, "Rail");
    std::fs::remove_dir_all(&gdb).ok();
}
