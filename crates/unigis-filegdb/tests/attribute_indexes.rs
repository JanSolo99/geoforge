//! Attribute indexes written by UniGIS equal those ArcGIS Pro writes
//! (testdata/gdb-reference/atx.gdb: table `t` with an index on every field type).

use std::path::{Path, PathBuf};

use unigis_core::Feature;
use unigis_filegdb::{Geodatabase, GeodatabaseWriter};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const INDEXES: &[(&str, &str)] = &[
    ("i_s", "S"),
    ("i_l", "L"),
    ("i_d", "D"),
    ("i_sh", "SH"),
    ("i_f", "F"),
    ("i_dt", "DT"),
    ("i_gi", "GI"),
    ("i_bi", "BI"),
    ("i_do", "DO"),
    ("i_to", "TO"),
    ("i_ts", "TS"),
    ("i_uniq_s", "S"),
];

#[test]
fn attribute_indexes_equal_arcgis() {
    let reference = root().join("testdata/gdb-reference/atx.gdb");
    let table = Geodatabase::open(&reference)
        .unwrap()
        .open_table("t")
        .unwrap();
    let fields: Vec<_> = table
        .fields()
        .iter()
        .filter(|f| !f.name.eq_ignore_ascii_case("OBJECTID"))
        .cloned()
        .collect();
    let rows: Vec<Feature> = table.features().map(|f| f.unwrap()).collect();

    let dir = std::env::temp_dir().join(format!("gf-atx-{}", std::process::id()));
    let out = dir.join("out.gdb");
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = GeodatabaseWriter::create(&out).unwrap();
    let mut t = w
        .create_table("t", None, false, false, &fields, None)
        .unwrap();
    for (name, field) in INDEXES {
        t.add_attribute_index(name, field).unwrap();
    }
    for r in &rows {
        t.write(r).unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();

    // Table `t` is a0000000b in ArcGIS's geodatabase (editing templates come first), a00000009 here.
    let mut problems = Vec::new();
    for (name, _) in INDEXES {
        let ours = std::fs::read(out.join(format!("a00000009.{name}.atx"))).unwrap();
        let theirs = std::fs::read(reference.join(format!("a0000000b.{name}.atx"))).unwrap();
        if ours != theirs {
            let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
            problems.push(format!(
                "{name}: ours {} bytes, ArcGIS {} bytes, first difference {first:?}",
                ours.len(),
                theirs.len()
            ));
        }
    }
    let ours = std::fs::read(out.join("a00000009.gdbindexes")).unwrap();
    let theirs = std::fs::read(reference.join("a0000000b.gdbindexes")).unwrap();
    if ours != theirs {
        problems.push(format!(
            ".gdbindexes: ours {} bytes, ArcGIS {} bytes",
            ours.len(),
            theirs.len()
        ));
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Multi-level trees: set `GF_BIG_GDB` to a geodatabase with table `t` (fields L long, S text(10),
/// D double) indexed as `i_l`, `i_s`, `i_d` by ArcGIS (300 000 rows are enough for three levels).
#[test]
fn large_attribute_indexes_equal_arcgis() {
    let Ok(reference) = std::env::var("GF_BIG_GDB") else {
        eprintln!("GF_BIG_GDB not set; skipped");
        return;
    };
    let reference = PathBuf::from(reference);
    let table = Geodatabase::open(&reference)
        .unwrap()
        .open_table("t")
        .unwrap();
    let fields: Vec<_> = table
        .fields()
        .iter()
        .filter(|f| !f.name.eq_ignore_ascii_case("OBJECTID"))
        .cloned()
        .collect();
    let dir = std::env::temp_dir().join(format!("gf-atx-big-{}", std::process::id()));
    let out = dir.join("out.gdb");
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = GeodatabaseWriter::create(&out).unwrap();
    let mut t = w
        .create_table("t", None, false, false, &fields, None)
        .unwrap();
    for (name, field) in [("i_l", "L"), ("i_s", "S"), ("i_d", "D")] {
        t.add_attribute_index(name, field).unwrap();
    }
    for r in table.features() {
        t.write(&r.unwrap()).unwrap();
    }
    w.finish_table(t).unwrap();
    w.finish().unwrap();
    let mut problems = Vec::new();
    for name in ["i_l", "i_s", "i_d"] {
        let ours = std::fs::read(out.join(format!("a00000009.{name}.atx"))).unwrap();
        let theirs = std::fs::read(reference.join(format!("a0000000b.{name}.atx"))).unwrap();
        if ours != theirs {
            let first = ours.iter().zip(&theirs).position(|(a, b)| a != b);
            problems.push(format!(
                "{name}: ours {} bytes, ArcGIS {} bytes, first difference {first:?}",
                ours.len(),
                theirs.len()
            ));
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(problems.is_empty(), "{problems:#?}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

/// Rebuilding every attribute index of an ArcGIS-created geodatabase (system tables,
/// editing templates and a user table with an index on a text field) from its rows
/// reproduces ArcGIS's `.atx` files byte for byte.
#[test]
fn rebuilt_indexes_equal_arcgis_in_a_whole_geodatabase() {
    let reference = root().join("testdata/gdb-reference/edit_base.gdb");
    let dir = std::env::temp_dir().join(format!("gf-atx-rebuild-{}", std::process::id()));
    let copy = dir.join("copy.gdb");
    copy_dir(&reference, &copy);
    let mut atx: Vec<String> = std::fs::read_dir(&reference)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".atx"))
        .collect();
    atx.sort();
    // Overwrite every index with junk, then rebuild from the tables.
    for n in &atx {
        std::fs::write(copy.join(n), b"junk").unwrap();
    }
    let mut tables: Vec<String> = atx
        .iter()
        .map(|n| n.split('.').next().unwrap().to_owned())
        .collect();
    tables.dedup();
    let mut built = 0;
    for t in &tables {
        built += unigis_filegdb::rebuild_attribute_indexes(copy.join(t))
            .unwrap()
            .len();
    }
    assert_eq!(built, atx.len());
    let mut problems = Vec::new();
    for n in &atx {
        let (ours, theirs) = (
            std::fs::read(copy.join(n)).unwrap(),
            std::fs::read(reference.join(n)).unwrap(),
        );
        if ours != theirs {
            problems.push(format!(
                "{n}: ours {} bytes, ArcGIS {} bytes, first difference {:?}",
                ours.len(),
                theirs.len(),
                ours.iter().zip(&theirs).position(|(a, b)| a != b)
            ));
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(problems.is_empty(), "{problems:#?}");
}
