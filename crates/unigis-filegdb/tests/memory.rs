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
