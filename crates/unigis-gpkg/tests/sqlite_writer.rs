//! The SQLite writer checked by SQLite itself: databases written at page sizes
//! 512, 4096 and 65536 (overflow pages, deep b-trees, indexes including unique
//! automatic ones, a schema too large for page 1) must pass
//! `PRAGMA integrity_check`, answer queries through their indexes like full
//! scans, and read back unchanged.

use std::path::{Path, PathBuf};
use std::process::Command;

use unigis_gpkg::sqlite::{Database, Row, SqlValue};
use unigis_gpkg::sqlite_writer::{Options, ValueRef, Writer};

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/gpkg")
        .join(name)
}

fn sqlite3(db: &Path, sql: &str) -> Option<String> {
    let out = Command::new("sqlite3").arg(db).arg(sql).output().ok()?;
    assert!(
        out.status.success(),
        "sqlite3 {sql}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn out_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("unigis-sqlite-writer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Copies every table of the test database, adds indexes, a large table and
/// many small ones, and writes it at `page_size`.
fn write_copy(path: &Path, page_size: usize) -> Vec<(String, Vec<Row>)> {
    let src = Database::open(&data("sqlite_4k.sqlite")).unwrap();
    let mut w = Writer::create(
        path,
        Options {
            page_size,
            application_id: 0x47504B47,
            user_version: 10400,
        },
    )
    .unwrap();
    let mut written = Vec::new();
    for t in src.tables().to_vec() {
        let rows = src.select(&t).unwrap();
        let mut tw = w.table(&t.name, &t.sql);
        for (rowid, values) in &rows {
            let refs: Vec<ValueRef> = values
                .iter()
                .zip(&t.columns)
                .map(|(v, c)| {
                    if c.rowid_alias {
                        ValueRef::Null
                    } else {
                        v.as_value_ref()
                    }
                })
                .collect();
            tw.push(*rowid, &refs).unwrap();
        }
        tw.finish().unwrap();
        if t.name == "mixed" {
            let col = |name: &str| t.columns.iter().position(|c| c.name == name).unwrap();
            let entries = |cols: &[usize]| -> Vec<(Vec<SqlValue>, i64)> {
                rows.iter()
                    .map(|(r, v)| (cols.iter().map(|&c| v[c].clone()).collect(), *r))
                    .collect()
            };
            w.index(
                "ix_i",
                "mixed",
                Some("CREATE INDEX ix_i ON mixed (i)"),
                entries(&[col("i")]),
            )
            .unwrap();
            w.index(
                "ix_t_r",
                "mixed",
                Some("CREATE INDEX ix_t_r ON mixed (t, r)"),
                entries(&[col("t"), col("r")]),
            )
            .unwrap();
            w.index(
                "ix_n",
                "mixed",
                Some("CREATE INDEX ix_n ON mixed (n)"),
                entries(&[col("n")]),
            )
            .unwrap();
        }
        written.push((t.name.clone(), rows));
    }
    // A table with text primary key and a unique column: automatic indexes.
    let mut keyed = Vec::new();
    {
        let mut tw = w.table(
            "keyed",
            "CREATE TABLE keyed (name TEXT NOT NULL PRIMARY KEY, code INTEGER UNIQUE, note TEXT)",
        );
        for k in 0..5000i64 {
            let name = format!(
                "name-{:05}-{}",
                (k * 7919) % 5000,
                "x".repeat((k % 300) as usize)
            );
            let values = vec![
                SqlValue::Text(name),
                SqlValue::Integer(10_000 - k),
                SqlValue::Text(format!("note {k}")),
            ];
            let refs: Vec<ValueRef> = values.iter().map(SqlValue::as_value_ref).collect();
            tw.push(k + 1, &refs).unwrap();
            keyed.push((k + 1, values));
        }
        tw.finish().unwrap();
    }
    let key_entries = |c: usize| -> Vec<(Vec<SqlValue>, i64)> {
        keyed
            .iter()
            .map(|(r, v)| (vec![v[c].clone()], *r))
            .collect()
    };
    w.index("sqlite_autoindex_keyed_1", "keyed", None, key_entries(0))
        .unwrap();
    w.index("sqlite_autoindex_keyed_2", "keyed", None, key_entries(1))
        .unwrap();
    written.push(("keyed".into(), keyed));
    // Many rows: three or more b-tree levels at small page sizes.
    let mut big = Vec::new();
    {
        let mut tw = w.table(
            "big",
            "CREATE TABLE big (id INTEGER PRIMARY KEY, a INTEGER, b REAL)",
        );
        for k in 0..200_000i64 {
            let rowid = k * 3 + 1;
            tw.push(
                rowid,
                &[
                    ValueRef::Null,
                    ValueRef::Integer(k * k),
                    ValueRef::Real(k as f64 / 7.0),
                ],
            )
            .unwrap();
            big.push((
                rowid,
                vec![
                    SqlValue::Integer(rowid),
                    SqlValue::Integer(k * k),
                    SqlValue::Real(k as f64 / 7.0),
                ],
            ));
        }
        tw.finish().unwrap();
    }
    written.push(("big".into(), big));
    // Enough schema entries to need more than page 1, and an empty table.
    for k in 0..300 {
        let name = format!("small_{k:03}");
        let tw = w.table(
            &name,
            &format!("CREATE TABLE {name} (a TEXT, b INTEGER, c REAL, d BLOB, e TEXT, f TEXT)"),
        );
        tw.finish().unwrap();
        written.push((name, Vec::new()));
    }
    w.schema_entry(
        "view",
        "v_big",
        "v_big",
        "CREATE VIEW v_big AS SELECT a FROM big",
    );
    w.finish().unwrap();
    written
}

#[test]
fn sqlite_accepts_what_we_write() {
    let dir = out_dir();
    let have_sqlite = Command::new("sqlite3").arg("-version").output().is_ok();
    for page_size in [512, 4096, 65536] {
        let path = dir.join(format!("w{page_size}.sqlite"));
        let written = write_copy(&path, page_size);
        // Our reader gets every row back.
        let db = Database::open(&path).unwrap();
        for (name, rows) in &written {
            let t = db
                .table(name)
                .unwrap_or_else(|| panic!("{page_size}: no {name}"));
            let back = db.select(t).unwrap();
            assert_eq!(back.len(), rows.len(), "{page_size} {name}");
            for (a, b) in back.iter().zip(rows) {
                assert_eq!(a.0, b.0, "{page_size} {name}");
                assert!(a.1 == b.1, "{page_size} {name} rowid {}", a.0);
            }
        }
        if !have_sqlite {
            eprintln!("sqlite3 not found; skipping the SQLite checks");
            continue;
        }
        assert_eq!(
            sqlite3(&path, "PRAGMA integrity_check").unwrap(),
            "ok",
            "{page_size}"
        );
        assert_eq!(
            sqlite3(&path, "PRAGMA application_id").unwrap(),
            "1196444487"
        );
        assert_eq!(sqlite3(&path, "PRAGMA user_version").unwrap(), "10400");
        assert_eq!(
            sqlite3(&path, "PRAGMA page_size").unwrap(),
            page_size.to_string()
        );
        // Index lookups agree with full scans.
        for (indexed, scan) in [
            (
                "SELECT count(*), total(i) FROM mixed INDEXED BY ix_i WHERE i > 100",
                "SELECT count(*), total(i) FROM mixed NOT INDEXED WHERE i > 100",
            ),
            (
                "SELECT count(*) FROM mixed INDEXED BY ix_t_r WHERE t >= 'b' AND t < 'c'",
                "SELECT count(*) FROM mixed NOT INDEXED WHERE t >= 'b' AND t < 'c'",
            ),
            (
                "SELECT note FROM keyed WHERE name = (SELECT name FROM keyed WHERE rowid = 4321)",
                "SELECT note FROM keyed NOT INDEXED WHERE rowid = 4321",
            ),
            (
                "SELECT name FROM keyed WHERE code = 7777",
                "SELECT name FROM keyed NOT INDEXED WHERE code = 7777",
            ),
            (
                "SELECT count(*), max(a) FROM v_big",
                "SELECT 200000, 39999600001",
            ),
        ] {
            assert_eq!(
                sqlite3(&path, indexed).unwrap(),
                sqlite3(&path, scan).unwrap(),
                "{page_size}: {indexed}"
            );
        }
        // A UNIQUE constraint is enforced through the automatic index.
        let dup = Command::new("sqlite3")
            .arg(&path)
            .arg("INSERT INTO keyed (name, code) VALUES ('new', 7777)")
            .output()
            .unwrap();
        assert!(!dup.status.success(), "{page_size}: duplicate accepted");
        // And SQLite can keep writing to the file.
        sqlite3(
            &path,
            "INSERT INTO big (a, b) VALUES (1, 2); DELETE FROM mixed WHERE id % 2 = 0;",
        );
        assert_eq!(
            sqlite3(&path, "PRAGMA integrity_check").unwrap(),
            "ok",
            "{page_size} after edits"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
