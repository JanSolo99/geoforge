//! Compares the FileGDB reader with GDAL's OpenFileGDB driver on every `.gdb` in
//! `testdata/large/filegdb` (from `scripts/fetch-testdata.sh`).

use std::path::{Path, PathBuf};

use geoforge_filegdb::{Geodatabase, Table};
use geoforge_testkit::{compare_feature, ogr_layer, ogr_layer_names, ogrinfo_available};

fn geodatabases(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.join("a00000001.gdbtable").is_file()
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
            {
                out.push(p);
            } else {
                out.extend(geodatabases(&p));
            }
        }
    }
    out.sort();
    out
}

/// Tables we deliberately do not read yet.
const UNSUPPORTED: &[(&str, &str)] = &[("Lake_labels", "compressed .cdf table")];

/// Sparse 64-bit OBJECTID tables: GDAL warns that its ObjectIDs "will not be
/// accurate", so features are matched in order instead of by OBJECTID.
fn gdal_oids_unreliable(gdb: &Path) -> bool {
    gdb.ends_with("objectid64/with_holes_8.gdb")
}

fn diff_table(gdb: &Path, name: &str, table: &Table) -> Result<Vec<String>, String> {
    let gdal = ogr_layer(gdb, Some(name))?;
    let names: Vec<&str> = table.fields().iter().map(|f| f.name.as_str()).collect();
    let mut diffs = Vec::new();
    let gdal_names: Vec<&str> = gdal.fields.iter().map(|f| f.name.as_str()).collect();
    if names != gdal_names {
        diffs.push(format!("fields: ours {names:?}, GDAL {gdal_names:?}"));
    }
    let mut count = 0;
    let by_order: Vec<_> = gdal.features.values().collect();
    for feature in table.features() {
        let feature = match feature {
            Ok(f) => f,
            Err(e) => {
                diffs.push(format!("read error: {e}"));
                continue;
            }
        };
        count += 1;
        let gdal_feature = if gdal_oids_unreliable(gdb) {
            by_order.get(count - 1).copied()
        } else {
            gdal.features.get(&feature.oid)
        };
        match gdal_feature {
            Some(g) => diffs.extend(compare_feature(&feature, &names, g)),
            None => diffs.push(format!("OBJECTID {} not reported by GDAL", feature.oid)),
        }
    }
    if count != gdal.features.len() {
        diffs.push(format!(
            "feature count: ours {count}, GDAL {}",
            gdal.features.len()
        ));
    }
    Ok(diffs)
}

#[test]
fn filegdb_matches_gdal() {
    if !ogrinfo_available() {
        eprintln!("skipping: ogrinfo not installed");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb");
    let gdbs = geodatabases(&root);
    let (mut tables, mut failures) = (0, 0);
    for path in &gdbs {
        let label = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let gdb = match Geodatabase::open(path) {
            Ok(g) => g,
            Err(e) => {
                failures += 1;
                eprintln!("✗ {label}: cannot open: {e}");
                continue;
            }
        };
        if let Ok(gdal_names) = ogr_layer_names(path) {
            let ours: Vec<&str> = gdb.entries().iter().map(|e| e.name.as_str()).collect();
            let missing: Vec<_> = gdal_names
                .iter()
                .filter(|n| !ours.contains(&n.as_str()))
                .collect();
            if !missing.is_empty() {
                failures += 1;
                eprintln!("✗ {label}: layers GDAL lists but we do not: {missing:?}");
            }
        }
        for entry in gdb.entries() {
            tables += 1;
            if let Some((_, why)) = UNSUPPORTED.iter().find(|(n, _)| *n == entry.name) {
                eprintln!("~ {label} / {}: skipped ({why})", entry.name);
                continue;
            }
            let diffs = match gdb.open_table(&entry.name) {
                Ok(t) => diff_table(path, &entry.name, &t).unwrap_or_else(|e| vec![e]),
                Err(e) => vec![format!("cannot open: {e}")],
            };
            if !diffs.is_empty() {
                failures += 1;
                eprintln!("✗ {label} / {}: {} differences", entry.name, diffs.len());
                diffs.iter().take(4).for_each(|d| eprintln!("    {d}"));
            }
        }
    }
    eprintln!(
        "{} geodatabases, {tables} tables, {failures} with differences",
        gdbs.len()
    );
    assert_eq!(failures, 0);
}

#[test]
fn sparse_64bit_objectids_are_exact() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/large/filegdb/objectid64/with_holes_8.gdb");
    let Ok(gdb) = Geodatabase::open(&path) else {
        eprintln!("skipping: run scripts/fetch-testdata.sh");
        return;
    };
    let oids = |name: &str| -> Vec<i64> {
        gdb.open_table(name)
            .unwrap()
            .features()
            .map(|f| f.unwrap().oid)
            .collect()
    };
    assert_eq!(oids("with_holes_8_a"), [123456]);
    assert_eq!(oids("with_holes_8_c"), [12345678]);
    assert_eq!(oids("with_holes_8_e"), [1234567890]);
    assert_eq!(
        oids("with_holes_8_f"),
        [123456, 1234567, 12345678, 123456789, 1234567890]
    );
}
