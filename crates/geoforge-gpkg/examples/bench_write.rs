//! Times the SQLite writer: `cargo run --release --example bench_write -- out.sqlite [rows]`.
use std::time::Instant;

use geoforge_gpkg::sqlite::SqlValue;
use geoforge_gpkg::sqlite_writer::{Options, ValueRef, Writer};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("output path");
    let rows: i64 = args.next().map_or(2_000_000, |s| s.parse().unwrap());
    let blob = vec![7u8; 100];
    let start = Instant::now();
    let mut w = Writer::create(path.as_ref(), Options::default()).unwrap();
    let mut t = w.table(
        "t",
        "CREATE TABLE t (id INTEGER PRIMARY KEY, a INTEGER, b REAL, c TEXT, d BLOB)",
    );
    let mut text = String::new();
    for k in 1..=rows {
        text.clear();
        use std::fmt::Write;
        write!(text, "feature number {k}").unwrap();
        t.push(
            k,
            &[
                ValueRef::Null,
                ValueRef::Integer((k * 7919) % rows),
                ValueRef::Real(k as f64 * 0.5),
                ValueRef::Text(&text),
                ValueRef::Blob(&blob),
            ],
        )
        .unwrap();
    }
    t.finish().unwrap();
    let table_done = start.elapsed();
    let index_start = Instant::now();
    let entries: Vec<(Vec<SqlValue>, i64)> = (1..=rows)
        .map(|k| (vec![SqlValue::Integer((k * 7919) % rows)], k))
        .collect();
    w.index("ix_a", "t", Some("CREATE INDEX ix_a ON t (a)"), entries)
        .unwrap();
    let index_done = index_start.elapsed();
    w.finish().unwrap();
    let all = start.elapsed();
    let size = std::fs::metadata(&path).unwrap().len();
    println!(
        "{rows} rows: table {:.2?} ({:.0} rows/s), index {:.2?}, total {:.2?}, {:.1} MB ({:.0} MB/s)",
        table_done,
        rows as f64 / table_done.as_secs_f64(),
        index_done,
        all,
        size as f64 / 1e6,
        size as f64 / 1e6 / all.as_secs_f64()
    );
}
