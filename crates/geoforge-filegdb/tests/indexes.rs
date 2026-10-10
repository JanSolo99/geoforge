//! Index lookups must never miss a row that a full scan finds. Checked for every
//! spatial (.spx) and plain-field attribute (.atx) index in the test data.

use std::path::{Path, PathBuf};

use geoforge_core::{Envelope, Value};
use geoforge_filegdb::{Geodatabase, Table, compare_values};

fn geodatabases(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.join("a00000001.gdbtable").is_file() {
            out.push(p);
        } else if p.is_dir() {
            out.extend(geodatabases(&p));
        }
    }
    out.sort();
    out
}

fn tables(root: &Path) -> Vec<(String, Table)> {
    let mut out = Vec::new();
    for gdb_path in geodatabases(root) {
        let Ok(gdb) = Geodatabase::open(&gdb_path) else {
            continue;
        };
        for e in gdb.entries() {
            if let Ok(t) = gdb.open_table(&e.name)
                && t.features().all(|f| f.is_ok())
            {
                out.push((
                    format!(
                        "{}/{}",
                        gdb_path.file_name().unwrap().to_string_lossy(),
                        e.name
                    ),
                    t,
                ));
            }
        }
    }
    out
}

fn root() -> Option<PathBuf> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb");
    p.is_dir().then_some(p)
}

fn intersects(a: &Envelope, b: &Envelope) -> bool {
    !a.is_empty() && a.xmin <= b.xmax && a.xmax >= b.xmin && a.ymin <= b.ymax && a.ymax >= b.ymin
}

#[test]
fn spatial_index_never_misses() {
    let Some(root) = root() else { return };
    let (mut indexes, mut queries, mut misses) = (0, 0, Vec::new());
    for (label, t) in tables(&root) {
        let envs: Vec<(i64, Envelope)> = t
            .features()
            .map(Result::unwrap)
            .filter_map(|f| f.geometry.map(|g| (f.oid, g.envelope())))
            .filter(|(_, e)| !e.is_empty())
            .collect();
        let Some(&(_, first)) = envs.first() else {
            continue;
        };
        let mut probes: Vec<Envelope> = envs
            .iter()
            .step_by((envs.len() / 40).max(1))
            .map(|(_, e)| *e)
            .collect();
        let mut all = first;
        envs.iter().for_each(|(_, e)| all.union(e));
        // A window over the middle quarter of the data, and the whole extent.
        let (w, h) = (all.xmax - all.xmin, all.ymax - all.ymin);
        probes.push(Envelope {
            xmin: all.xmin + w * 0.375,
            xmax: all.xmin + w * 0.625,
            ymin: all.ymin + h * 0.375,
            ymax: all.ymin + h * 0.625,
            z: None,
            m: None,
        });
        probes.push(all);

        let Ok(Some(_)) = t.spatial_index_candidates(&first) else {
            continue;
        };
        indexes += 1;
        for probe in probes {
            queries += 1;
            let found = t.spatial_index_candidates(&probe).unwrap().unwrap();
            for (oid, e) in &envs {
                if intersects(e, &probe) && found.binary_search(&(*oid as u64)).is_err() {
                    misses.push(format!("{label}: OID {oid} missed for {probe:?}"));
                }
            }
        }
    }
    eprintln!(
        "{indexes} spatial indexes, {queries} queries, {} misses",
        misses.len()
    );
    misses.iter().take(10).for_each(|m| eprintln!("✗ {m}"));
    assert!(indexes > 20 && misses.is_empty());
}

#[test]
fn attribute_index_never_misses() {
    let Some(root) = root() else { return };
    let (mut indexes, mut queries, mut misses) = (0, 0, Vec::new());
    for (label, t) in tables(&root) {
        for def in t.indexes().to_vec() {
            let Some(i) = t
                .fields()
                .iter()
                .position(|f| f.name.eq_ignore_ascii_case(&def.expression))
            else {
                continue;
            };
            let rows: Vec<(i64, Value)> = t
                .features()
                .map(Result::unwrap)
                .map(|f| (f.oid, f.attributes[i].clone()))
                .collect();
            let mut values: Vec<Value> = rows
                .iter()
                .map(|(_, v)| v.clone())
                .filter(|v| !v.is_null())
                .collect();
            values.sort_by(|a, b| compare_values(a, b).unwrap_or(std::cmp::Ordering::Equal));
            values.dedup();
            let Some(first) = values.first().cloned() else {
                continue;
            };
            let Ok(Some(_)) = t.attribute_index_candidates(&def.expression, &first, &first) else {
                misses.push(format!(
                    "{label}: index {} on {} unusable",
                    def.name, def.expression
                ));
                continue;
            };
            indexes += 1;
            let step = (values.len() / 50).max(1);
            let mut probes: Vec<(Value, Value)> = values
                .iter()
                .step_by(step)
                .map(|v| (v.clone(), v.clone()))
                .collect();
            probes.push((
                values[values.len() / 4].clone(),
                values[values.len() * 3 / 4].clone(),
            ));
            for (lo, hi) in probes {
                queries += 1;
                let found = t
                    .attribute_index_candidates(&def.expression, &lo, &hi)
                    .unwrap()
                    .unwrap();
                for (oid, v) in &rows {
                    let inside = compare_values(v, &lo).is_some_and(|o| o.is_ge())
                        && compare_values(v, &hi).is_some_and(|o| o.is_le());
                    if inside && found.binary_search(&(*oid as u64)).is_err() {
                        misses.push(format!(
                            "{label}.{}: OID {oid} ({v:?}) missed for {lo:?}..{hi:?}",
                            def.expression
                        ));
                    }
                }
            }
        }
    }
    eprintln!(
        "{indexes} attribute indexes, {queries} queries, {} misses",
        misses.len()
    );
    misses.iter().take(10).for_each(|m| eprintln!("✗ {m}"));
    assert!(indexes > 10 && misses.is_empty());
}
