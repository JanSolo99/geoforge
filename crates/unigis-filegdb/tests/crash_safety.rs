//! Edits of an existing geodatabase are transactions: a process killed at any step of an
//! edit leaves a geodatabase that, once opened again, is either exactly the original (byte
//! for byte) or carries the whole edit. The child process is this test binary, killed by
//! the `GF_FILEGDB_CRASH_AT` hook at step 1, 2, ... until the edit completes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use unigis_core::{Feature, Field, FieldType, Geometry, Value};
use unigis_filegdb::{
    Geodatabase, GeodatabaseWriter, TableEditor, delete_item, rename_item, rewrite_table,
};

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

/// Every file of the geodatabase (sub-directories included) by relative path.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(dir).unwrap().display().to_string();
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

fn well(id: i32, x: f64, y: f64) -> Feature {
    Feature {
        oid: 0,
        geometry: Some(Geometry::point(x, y)),
        attributes: vec![Value::Integer(id), Value::Double(7.25)],
    }
}

/// The edits, run in the child process.
fn run(op: &str, gdb: &Path) {
    let base = Geodatabase::open(gdb).unwrap().table_base("wells").unwrap();
    match op {
        "edit" => {
            let mut ed = TableEditor::open(&base).unwrap();
            ed.insert(&well(900, 510_000.0, 5_010_000.0)).unwrap();
            ed.insert(&well(901, 510_050.0, 5_010_050.0)).unwrap();
            ed.set_geometry(1, Some(Geometry::point(509_970.0, 5_010_000.0)))
                .unwrap();
            ed.set_value(3, "DEPTH", Value::Double(99.5)).unwrap();
            ed.delete(2).unwrap();
            ed.finish().unwrap();
        }
        "rewrite" => {
            let table = Geodatabase::open(gdb).unwrap().open_table("wells").unwrap();
            let mut fields = table.fields().to_vec();
            fields.push(Field {
                length: 12,
                ..Field::new("NOTE", FieldType::String)
            });
            let rows: Vec<(Option<u64>, Feature)> = table
                .features()
                .map(|f| {
                    let mut f = f.unwrap();
                    f.attributes.push(Value::String(format!("w{}", f.oid)));
                    (Some(f.oid as u64), f)
                })
                .collect();
            drop(table);
            rewrite_table(&base, &fields, &rows).unwrap();
        }
        "create" => {
            let mut w = GeodatabaseWriter::open_existing(gdb).unwrap();
            let mut t = w
                .create_table(
                    "notes",
                    None,
                    false,
                    false,
                    &[Field {
                        length: 20,
                        ..Field::new("TXT", FieldType::String)
                    }],
                    None,
                )
                .unwrap();
            for i in 0..5 {
                t.write(&Feature {
                    oid: 0,
                    geometry: None,
                    attributes: vec![Value::String(format!("note {i}"))],
                })
                .unwrap();
            }
            w.finish_table(t).unwrap();
            w.finish().unwrap();
        }
        "delete" => {
            delete_item(gdb, "wells").unwrap();
        }
        "rename" => {
            rename_item(gdb, "wells", "boreholes").unwrap();
        }
        other => panic!("unknown op {other}"),
    }
}

/// Whether `gdb` holds the complete result of `op`.
fn is_done(op: &str, gdb: &Path) -> bool {
    let g = Geodatabase::open(gdb).unwrap();
    let names: Vec<String> = g.entries().iter().map(|e| e.name.to_lowercase()).collect();
    match op {
        "edit" => {
            let t = g.open_table("wells").unwrap();
            let rows: Vec<Feature> = t.features().map(|f| f.unwrap()).collect();
            rows.len() == 51
                && rows.iter().all(|f| f.oid != 2)
                && rows.iter().any(|f| f.attributes[0] == Value::Integer(901))
                && rows
                    .iter()
                    .any(|f| f.oid == 3 && f.attributes[1] == Value::Double(99.5))
                && t.features_in_extent(&unigis_core::Envelope {
                    xmin: 509_900.0,
                    ymin: 5_009_900.0,
                    xmax: 510_100.0,
                    ymax: 5_010_100.0,
                    z: None,
                    m: None,
                })
                .unwrap()
                .len()
                    == 3
        }
        "rewrite" => {
            let t = g.open_table("wells").unwrap();
            t.fields().iter().any(|f| f.name == "NOTE")
                && t.features().all(|f| {
                    let f = f.unwrap();
                    f.attributes[2] == Value::String(format!("w{}", f.oid))
                })
        }
        "create" => {
            names.contains(&"notes".to_string()) && g.open_table("notes").unwrap().row_count() == 5
        }
        "delete" => !names.contains(&"wells".to_string()),
        "rename" => {
            names.contains(&"boreholes".to_string()) && !names.contains(&"wells".to_string())
        }
        _ => unreachable!(),
    }
}

#[test]
#[ignore = "run by crash_at_every_step as a child process"]
fn crash_child() {
    let (Ok(op), Ok(gdb)) = (std::env::var("GF_TEST_OP"), std::env::var("GF_TEST_GDB")) else {
        return;
    };
    run(&op, Path::new(&gdb));
}

#[test]
fn crash_at_every_step() {
    let dir = std::env::temp_dir().join(format!("gf-crash-{}", std::process::id()));
    let gdb = dir.join("t.gdb");
    for op in ["edit", "rewrite", "create", "delete", "rename"] {
        copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &gdb);
        let before = snapshot(&gdb);
        let (mut rolled_back, mut completed) = (0, 0);
        for at in 1.. {
            copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &gdb);
            let out = Command::new(std::env::current_exe().unwrap())
                .args(["crash_child", "--exact", "--ignored", "--test-threads=1"])
                .env("GF_TEST_OP", op)
                .env("GF_TEST_GDB", &gdb)
                .env("GF_FILEGDB_CRASH_AT", at.to_string())
                .output()
                .unwrap();
            let crashed = out.status.code() == Some(86);
            assert!(
                crashed || out.status.success(),
                "{op} at {at}: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            // Opening recovers; afterwards there is no journal left.
            Geodatabase::open(&gdb).unwrap();
            assert!(!gdb.join(".gf-journal").exists(), "{op} at {at}");
            let after = snapshot(&gdb);
            let was_rolled_back = after == before;
            if was_rolled_back {
                rolled_back += 1;
            } else {
                assert!(
                    is_done(op, &gdb),
                    "{op}: crash at step {at} left a partial edit"
                );
                completed += 1;
            }
            if !crashed {
                assert!(is_done(op, &gdb), "{op}: the edit did not complete");
                break;
            }
            // After recovery the geodatabase can be edited again.
            if was_rolled_back && at % 7 == 0 {
                run(op, &gdb);
                assert!(is_done(op, &gdb));
            }
        }
        assert!(
            rolled_back > 2 && completed >= 1,
            "{op}: {rolled_back} {completed}"
        );
        eprintln!("{op}: {rolled_back} crash points rolled back, {completed} completed");
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_dropped_editor_rolls_back() {
    let dir = std::env::temp_dir().join(format!("gf-drop-{}", std::process::id()));
    let gdb = dir.join("t.gdb");
    copy_dir(&root().join("testdata/gdb-reference/edit_fc.gdb"), &gdb);
    let before = snapshot(&gdb);
    let base = Geodatabase::open(&gdb)
        .unwrap()
        .table_base("wells")
        .unwrap();
    {
        let mut ed = TableEditor::open(&base).unwrap();
        ed.insert(&well(5, 1.0, 2.0)).unwrap();
        ed.delete(4).unwrap();
        // An error part-way through an edit drops the editor.
        assert!(ed.update(4, &well(4, 0.0, 0.0)).is_err());
    }
    assert_eq!(snapshot(&gdb), before);
    // A failing rewrite (two rows with one OBJECTID) leaves the table alone too.
    let table = Geodatabase::open(&gdb)
        .unwrap()
        .open_table("wells")
        .unwrap();
    let f = table.get(1).unwrap().unwrap();
    let fields = table.fields().to_vec();
    drop(table);
    let bad = vec![(Some(1), f.clone()), (Some(1), f)];
    assert!(rewrite_table(&base, &fields, &bad).is_err());
    assert_eq!(snapshot(&gdb), before);
    std::fs::remove_dir_all(&dir).ok();
}
