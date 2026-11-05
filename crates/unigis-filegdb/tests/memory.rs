//! A geodatabase read from memory (a browser upload) must read exactly as it does from disk:
//! the same catalog, feature datasets, domains, fields, rows and index lookups.

use std::path::{Path, PathBuf};

use unigis_core::Envelope;
use unigis_filegdb::Geodatabase;

fn geodatabases(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
            && p.join("a00000001.gdbtable").is_file()
        {
            out.push(p);
        } else {
            geodatabases(&p, out);
        }
    }
}

/// The folder's files, named as a browser folder upload names them (`Name.gdb/file`).
fn files(gdb: &Path) -> Vec<(String, Vec<u8>)> {
    let folder = gdb.file_name().unwrap().to_string_lossy().into_owned();
    std::fs::read_dir(gdb)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            let name = format!("{folder}/{}", e.file_name().to_string_lossy());
            (name, std::fs::read(e.path()).unwrap())
        })
        .collect()
}

#[test]
fn memory_reads_match_disk() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let mut gdbs = Vec::new();
    geodatabases(&root, &mut gdbs);
    gdbs.sort();
    assert!(!gdbs.is_empty(), "no geodatabases under {}", root.display());
    let mut tables = 0;
    for path in &gdbs {
        // Read-only on both sides: `Geodatabase::open` may roll back an interrupted edit.
        let disk = Geodatabase::open_in(Default::default(), path.clone())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let name = path.file_name().unwrap();
        let mem = Geodatabase::from_files(name, files(path))
            .unwrap_or_else(|e| panic!("{} from memory: {e}", path.display()));
        assert!(mem.store().is_memory());
        assert_eq!(mem.entries(), disk.entries(), "{}", path.display());
        assert_eq!(mem.feature_datasets(), disk.feature_datasets());
        assert_eq!(mem.domains(), disk.domains());
        for entry in disk.entries() {
            let (Ok(a), b) = (disk.open_table(&entry.name), mem.open_table(&entry.name)) else {
                // Tables the disk reader rejects (compressed .cdf) must fail from memory too.
                assert!(mem.open_table(&entry.name).is_err(), "{}", entry.name);
                continue;
            };
            let b = b.unwrap_or_else(|e| panic!("{} / {}: {e}", path.display(), entry.name));
            let label = format!("{} / {}", path.display(), entry.name);
            assert_eq!(a.fields(), b.fields(), "{label}");
            assert_eq!(a.row_count(), b.row_count(), "{label}");
            assert_eq!(a.indexes(), b.indexes(), "{label}");
            // Compared as text: M values may be NaN, which never equals itself.
            let rows = |t: &unigis_filegdb::Table| -> Vec<String> {
                t.features().map(|f| format!("{f:?}")).collect()
            };
            assert_eq!(rows(&a), rows(&b), "{label}");
            // The spatial index is read through the same store.
            let all = Envelope {
                xmin: f64::MIN,
                ymin: f64::MIN,
                xmax: f64::MAX,
                ymax: f64::MAX,
                z: None,
                m: None,
            };
            assert_eq!(
                a.spatial_index_candidates(&all).map_err(|e| e.to_string()),
                b.spatial_index_candidates(&all).map_err(|e| e.to_string()),
                "{label}"
            );
            tables += 1;
        }
    }
    assert!(tables > 0);
    eprintln!(
        "{} geodatabases, {tables} tables read identically from memory",
        gdbs.len()
    );
}

#[test]
fn missing_catalog_is_an_error_not_a_panic() {
    assert!(Geodatabase::from_files("x.gdb", Vec::new()).is_err());
    let junk = vec![("x.gdb/a00000001.gdbtable".to_string(), vec![0xff; 7])];
    assert!(Geodatabase::from_files("x.gdb", junk).is_err());
}

/// Every catalog item is listed with its type's name, as ArcGIS's Catalog shows them.
#[test]
fn items_carry_their_type_names() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let mut gdbs = Vec::new();
    geodatabases(&root, &mut gdbs);
    let mut kinds = std::collections::BTreeSet::new();
    for path in &gdbs {
        let g = Geodatabase::open_in(Default::default(), path.clone()).unwrap();
        for e in g.entries() {
            let item = g
                .items()
                .iter()
                .find(|i| i.name.eq_ignore_ascii_case(&e.name));
            if let Some(item) = item {
                // Tables back other items too (attributed relationship classes, raster catalogs).
                assert!(
                    !item.kind.is_empty() && !item.kind.starts_with('{'),
                    "{}: {}",
                    path.display(),
                    e.name
                );
                assert_eq!(item.feature_dataset, e.feature_dataset, "{}", e.name);
            }
        }
        for fds in g.feature_datasets() {
            assert!(
                g.items()
                    .iter()
                    .any(|i| i.name == *fds && i.kind == "Feature Dataset"),
                "{fds}"
            );
        }
        kinds.extend(g.items().iter().map(|i| i.kind.clone()));
    }
    for k in [
        "Workspace",
        "Feature Class",
        "Table",
        "Feature Dataset",
        "Relationship Class",
        "Coded Value Domain",
        "Range Domain",
    ] {
        assert!(kinds.contains(k), "no {k} among {kinds:?}");
    }
    assert!(
        !kinds.iter().any(|k| k.starts_with('{')),
        "unnamed types: {kinds:?}"
    );
}

/// The writer on a geodatabase mounted in memory (an upload being edited) gives the same files
/// as on disk: a field added, an item renamed, another created, one deleted.
#[test]
fn edits_in_memory_match_edits_on_disk() {
    use unigis_core::{Field, FieldType};
    use unigis_filegdb::vfs;
    let src =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/gdb-reference/edit_fc.gdb");
    let disk = std::env::temp_dir()
        .join(format!("unigis-vfs-disk-{}", std::process::id()))
        .join("edit_fc.gdb");
    std::fs::remove_dir_all(disk.parent().unwrap()).ok();
    std::fs::create_dir_all(&disk).unwrap();
    for (name, bytes) in files(&src) {
        std::fs::write(disk.join(Path::new(&name).file_name().unwrap()), bytes).unwrap();
    }
    let mem = PathBuf::from(format!(
        "/unigis-memory-test/{}/edit_fc.gdb",
        std::process::id()
    ));
    vfs::mount(&mem, files(&src));
    let edit = |dir: &Path| {
        let g = Geodatabase::open(dir).unwrap();
        let base = g.table_base("wells").unwrap();
        unigis_filegdb::add_field(&base, &Field::new("DEPTH_M", FieldType::Double)).unwrap();
        unigis_filegdb::rename_item(dir, "wells", "Wells2").unwrap();
        let mut w = unigis_filegdb::GeodatabaseWriter::open_existing(dir).unwrap();
        let t = w
            .create_table(
                "notes",
                None,
                false,
                false,
                &[Field::new("TEXT", FieldType::String)],
                None,
            )
            .unwrap();
        w.finish_table(t).unwrap();
        w.finish().unwrap();
        unigis_filegdb::delete_item(dir, "notes").unwrap();
    };
    edit(&disk);
    edit(&mem);
    let on_disk: std::collections::BTreeMap<String, Vec<u8>> = files(&disk)
        .into_iter()
        .map(|(n, b)| (n.rsplit('/').next().unwrap().to_string(), b))
        .collect();
    let in_memory: std::collections::BTreeMap<String, Vec<u8>> =
        vfs::unmount(&mem).into_iter().collect();
    assert_eq!(
        on_disk.keys().collect::<Vec<_>>(),
        in_memory.keys().collect::<Vec<_>>()
    );
    // Identical, but for the random GUIDs new items get (GDB_Items, GDB_ItemRelationships).
    let mask = |t: String| {
        let mut out = String::new();
        let mut rest = t.as_str();
        while let Some(i) = rest.find('{') {
            out.push_str(&rest[..i]);
            let tail = &rest[i..];
            let guid = tail.len() >= 38
                && tail.as_bytes()[37] == b'}'
                && tail[1..37]
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || c == '-');
            out.push_str(if guid { "{GUID}" } else { "{" });
            rest = &tail[if guid { 38 } else { 1 }..];
        }
        out + rest
    };
    let dump = |files: &std::collections::BTreeMap<String, Vec<u8>>, name: &str| -> Vec<String> {
        let g = Geodatabase::from_files("x.gdb", files.clone()).unwrap();
        let id = g
            .catalog()
            .iter()
            .find(|(_, id)| format!("a{id:08x}.gdbtable") == name)
            .map(|(_, id)| *id)
            .unwrap();
        unigis_filegdb::Table::open_in(g.store(), g.path().join(format!("a{id:08x}")))
            .unwrap()
            .features()
            .map(|f| mask(format!("{f:?}")))
            .collect()
    };
    for (name, bytes) in &on_disk {
        if in_memory[name] != *bytes {
            assert!(
                ["a00000004.gdbtable", "a00000006.gdbtable"].contains(&name.as_str()),
                "{name} differs"
            );
            assert_eq!(
                dump(&on_disk, name),
                dump(&in_memory, name),
                "{name}: rows differ beyond GUIDs"
            );
        }
    }
    // And the memory result reads back.
    let g = Geodatabase::from_files("edit_fc.gdb", in_memory.clone()).unwrap();
    let t = g.open_table("Wells2").unwrap();
    assert!(t.fields().iter().any(|f| f.name == "DEPTH_M"));
    std::fs::remove_dir_all(disk.parent().unwrap()).ok();
}
