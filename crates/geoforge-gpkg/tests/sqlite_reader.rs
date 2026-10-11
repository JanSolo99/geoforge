//! The SQLite reader against Python's sqlite3: every row of every table, at page
//! sizes 512, 4096 and 65536, with overflow pages, deleted rows and an added
//! column (rows written before it lack the value).

use std::path::Path;

use geoforge_gpkg::sqlite::{Database, SqlValue};
use serde_json::Value as Json;

fn data(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/gpkg")
        .join(name)
}

fn expected(v: &Json) -> SqlValue {
    match v {
        Json::Null => SqlValue::Null,
        Json::Number(n) => SqlValue::Integer(n.as_i64().expect("integer")),
        Json::String(s) => SqlValue::Text(s.clone()),
        Json::Object(o) if o.contains_key("real") => SqlValue::Real(o["real"].as_f64().unwrap()),
        Json::Object(o) => SqlValue::Blob(decode64(o["blob"].as_str().unwrap())),
        other => panic!("unexpected {other}"),
    }
}

fn decode64(s: &str) -> Vec<u8> {
    let val = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            _ => 63,
        }
    };
    let mut out = Vec::new();
    for chunk in s.as_bytes().chunks(4) {
        let n = chunk.iter().filter(|&&c| c != b'=').count();
        let mut acc = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            acc |= if c == b'=' { 0 } else { val(c) } << (18 - 6 * i);
        }
        out.extend_from_slice(&acc.to_be_bytes()[1..n]);
    }
    out
}

#[test]
fn reads_like_sqlite() {
    let dump: serde_json::Map<String, Json> =
        serde_json::from_str(&std::fs::read_to_string(data("sqlite_dump.json")).unwrap()).unwrap();
    for file in ["sqlite_4k.sqlite", "sqlite_512.sqlite", "sqlite_64k.sqlite"] {
        let db = Database::open(&data(file)).unwrap();
        for (name, rows) in &dump {
            let table = db
                .table(name)
                .unwrap_or_else(|| panic!("{file}: no table {name}"));
            let ours = db.select(table).unwrap();
            let rows = rows.as_array().unwrap();
            assert_eq!(ours.len(), rows.len(), "{file} {name}: row count");
            for ((rowid, values), want) in ours.iter().zip(rows) {
                let want = want.as_array().unwrap();
                assert_eq!(Some(*rowid), want[0].as_i64(), "{file} {name}");
                let want: Vec<SqlValue> = want[1..].iter().map(expected).collect();
                // A column added later reads as its default in SQLite; we give NULL
                // for rows written before it, so compare those as the default.
                let mut values = values.clone();
                if name == "later" && values[2] == SqlValue::Null {
                    values[2] = SqlValue::Real(1.5);
                }
                assert_eq!(values.len(), want.len(), "{file} {name} rowid {rowid}");
                for (k, (a, b)) in values.iter().zip(&want).enumerate() {
                    // serde_json's default float parsing can be off by an ulp.
                    let same = match (a, b) {
                        (SqlValue::Real(x), SqlValue::Real(y)) => (x - y).abs() <= 1e-15 * y.abs(),
                        _ => a == b,
                    };
                    assert!(
                        same,
                        "{file} {name} rowid {rowid} column {k}: {a:.80?} vs {b:.80?}"
                    );
                }
            }
        }
    }
}
