//! Adding and removing attribute indexes leaves the index files as ArcGIS Pro does
//! (testdata/gdb-reference/index_ops: names, sizes and FNV-1a hash of the `.gdbindexes`, `.atx` and
//! `.spx` files of the two tables of index_base.gdb after each operation).

use std::path::{Path, PathBuf};

use serde_json::Value as Json;
use unigis_filegdb as g;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn fnv(data: &[u8]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn snapshot(dir: &Path) -> Json {
    let mut out = serde_json::Map::new();
    for stem in ["a0000000b", "a0000000c"] {
        let mut files = serde_json::Map::new();
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| {
                n.starts_with(&format!("{stem}."))
                    && [".atx", ".spx", ".gdbindexes"]
                        .iter()
                        .any(|x| n.ends_with(x))
            })
            .collect();
        names.sort();
        for n in names {
            let data = std::fs::read(dir.join(&n)).unwrap();
            files.insert(
                n[10..].to_owned(),
                serde_json::json!({
                    "size": data.len(),
                    "fnv": fnv(&data),
                }),
            );
        }
        out.insert(stem.to_owned(), Json::Object(files));
    }
    Json::Object(out)
}

fn codes(message: &str) -> Vec<String> {
    message
        .match_indices("ERROR ")
        .map(|(at, _)| message[at..].chars().take(12).collect())
        .collect()
}

#[test]
fn attribute_index_operations_equal_arcgis() {
    let reference = root().join("testdata/gdb-reference");
    let manifest: Json =
        serde_json::from_slice(&std::fs::read(reference.join("index_ops/ops.json")).unwrap())
            .unwrap();
    let mut failures = Vec::new();
    for (op, spec) in manifest["ops"].as_object().unwrap() {
        let s = spec["spec"].as_array().unwrap();
        let tool = s[0].as_str().unwrap();
        let multi = tool == "AddIndex" && s[2].as_array().is_some_and(|f| f.len() != 1);
        if !matches!(tool, "AddIndex" | "RemoveIndex") || multi {
            continue; // spatial indexes and multi-field indexes are not done
        }
        let work = std::env::temp_dir().join(format!("gf-index-{op}-{}.gdb", std::process::id()));
        std::fs::remove_dir_all(&work).ok();
        copy_dir(&reference.join("index_base.gdb"), &work);
        let table = s[1].as_str().unwrap();
        let result = if tool == "AddIndex" {
            let field = s[2][0].as_str().unwrap();
            g::add_attribute_index(&work, table, field, s[3].as_str().unwrap_or(""))
        } else {
            g::remove_attribute_index(&work, table, s[2].as_str().unwrap())
        };
        let ours = snapshot(&work);
        std::fs::remove_dir_all(&work).ok();
        let arcgis = manifest["arcgis"][op].as_str().unwrap();
        if arcgis == "ok" {
            if let Err(e) = &result {
                failures.push(format!("{op}: ArcGIS accepts, we fail: {e}"));
                continue;
            }
        } else {
            match &result {
                Ok(()) => failures.push(format!("{op}: ArcGIS fails ({arcgis}), we accept")),
                Err(e) => {
                    let ours = e.to_string();
                    if !codes(arcgis).iter().any(|c| ours.contains(c.as_str())) {
                        failures.push(format!("{op}: error {e}, ArcGIS {arcgis}"));
                    }
                }
            }
        }
        let want: Json = serde_json::from_slice(
            &std::fs::read(reference.join(format!("index_ops/{op}.json"))).unwrap(),
        )
        .unwrap();
        for stem in ["a0000000b", "a0000000c"] {
            // Only the file lists and sizes: the table files themselves are rewritten.
            let (a, b) = (&ours[stem], &want[stem]);
            for (name, w) in b.as_object().unwrap() {
                match a.get(name) {
                    None => failures.push(format!("{op}: {stem}.{name} missing")),
                    Some(o) if o != w => {
                        failures.push(format!("{op}: {stem}.{name} differs: ours {o}, ArcGIS {w}"))
                    }
                    _ => {}
                }
            }
            for name in a.as_object().unwrap().keys() {
                if b.get(name).is_none() {
                    failures.push(format!("{op}: {stem}.{name} should not exist"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
