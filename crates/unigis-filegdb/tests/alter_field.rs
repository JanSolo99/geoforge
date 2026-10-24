//! Alter Field on a file geodatabase table leaves the table, its item definition and its rows as
//! ArcGIS Pro leaves them (testdata/gdb-reference/alter_ops: ArcGIS's state after each operation on
//! alter_base.gdb, as scripts/gdb-snapshot.py makes it through GDAL; `ops.json` lists the
//! operations and what ArcGIS answered).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::FieldType;
use unigis_filegdb as g;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn snapshot(gdb: &Path) -> Option<Json> {
    let out = Command::new("python3")
        .arg(root().join("scripts/gdb-snapshot.py"))
        .arg(gdb)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| serde_json::from_slice(&out.stdout).ok())?
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn field_type(name: &str) -> FieldType {
    match name {
        "TEXT" => FieldType::String,
        "LONG" => FieldType::Integer,
        "SHORT" => FieldType::SmallInteger,
        "DOUBLE" => FieldType::Double,
        "DATE" => FieldType::Date,
        "GUID" => FieldType::Guid,
        other => panic!("field type {other}"),
    }
}

/// The rows CSV with `500002.0` written as `500002`: GDAL before 3.9 prints whole coordinates
/// without the fraction, so the stored snapshots (made with a newer GDAL) are compared this way.
fn plain_numbers(rows: &Json) -> String {
    let chars: Vec<char> = rows.to_string().chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let whole = i > 0 && chars[i - 1].is_ascii_digit();
        let zero_fraction = chars[i] == '.'
            && chars.get(i + 1) == Some(&'0')
            && !chars.get(i + 2).is_some_and(char::is_ascii_digit);
        if whole && zero_fraction {
            i += 2;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// Every `ERROR nnnnnn` code of a message.
fn codes(message: &str) -> Vec<String> {
    message
        .match_indices("ERROR ")
        .map(|(at, _)| message[at..].chars().take(12).collect())
        .collect()
}

#[test]
fn alter_field_equals_arcgis() {
    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let reference = root().join("testdata/gdb-reference");
    let manifest: Json =
        serde_json::from_slice(&std::fs::read(reference.join("alter_ops/ops.json")).unwrap())
            .unwrap();
    let ops = manifest["ops"].as_object().unwrap();
    let mut failures = Vec::new();
    for (op, a) in ops {
        let work = std::env::temp_dir().join(format!("gf-alter-{op}-{}.gdb", std::process::id()));
        std::fs::remove_dir_all(&work).ok();
        copy_dir(&reference.join("alter_base.gdb"), &work);
        let text = |k: &str| a.get(k).and_then(Json::as_str);
        let alteration = g::FieldAlteration {
            new_name: text("new_name"),
            alias: text("alias"),
            clear_alias: text("clear").is_some(),
            new_type: text("type").map(field_type),
            length: a.get("length").and_then(Json::as_u64).map(|l| l as u32),
            nullable: text("nullable").map(|n| n == "NULLABLE"),
        };
        let result = if let Some(rows) = a.get("rows").and_then(Json::as_array) {
            // Alter Fields: `field, new name, alias, type, length, nullable, clear` per row.
            let cells: Vec<Vec<Option<String>>> = rows
                .iter()
                .map(|r| {
                    (0..7)
                        .map(|i| match &r[i] {
                            Json::String(s) => Some(s.clone()),
                            Json::Number(n) => Some(n.to_string()),
                            _ => None,
                        })
                        .collect()
                })
                .collect();
            let rows: Vec<(&str, g::FieldAlteration)> = cells
                .iter()
                .map(|c| {
                    // `#` skips a cell, except for the alias, which is kept literally.
                    let get = |i: usize| c[i].as_deref().filter(|v| i == 2 || *v != "#");
                    (
                        get(0).unwrap(),
                        g::FieldAlteration {
                            new_name: get(1),
                            alias: get(2),
                            clear_alias: get(6).is_some(),
                            new_type: get(3).map(field_type),
                            length: get(4).and_then(|l| l.parse().ok()),
                            nullable: get(5).map(|n| n == "NULLABLE"),
                        },
                    )
                })
                .collect();
            g::alter_fields(&work, text("table").unwrap(), &rows)
        } else {
            g::alter_field(
                &work,
                text("table").unwrap(),
                text("field").unwrap(),
                &alteration,
            )
        };
        let ours = snapshot(&work);
        std::fs::remove_dir_all(&work).ok();
        let Some(ours) = ours else { return };
        let arcgis = manifest["arcgis"][op].as_str().unwrap();
        let want: Json = serde_json::from_slice(
            &std::fs::read(reference.join(format!("alter_ops/{op}.json"))).unwrap(),
        )
        .unwrap();
        if arcgis == "ok" {
            if let Err(e) = &result {
                failures.push(format!("{op}: ArcGIS accepts, we fail: {e}"));
                continue;
            }
        } else {
            match &result {
                Ok(()) => {
                    failures.push(format!("{op}: ArcGIS fails ({arcgis}), we accept"));
                    continue;
                }
                Err(e) => {
                    let ours = e.to_string();
                    if !codes(arcgis).iter().any(|c| ours.contains(c.as_str())) {
                        failures.push(format!("{op}: error {e}, ArcGIS {arcgis}"));
                    }
                }
            }
        }
        for key in ["fields", "rows", "defaults", "catalog", "rels", "tables"] {
            let same = if key == "rows" {
                plain_numbers(&ours[key]) == plain_numbers(&want[key])
            } else {
                ours[key] == want[key]
            };
            if !same {
                failures.push(format!(
                    "{op}: {key} differ\n ours:   {}\n ArcGIS: {}",
                    ours[key].to_string().chars().take(900).collect::<String>(),
                    want[key].to_string().chars().take(900).collect::<String>()
                ));
            }
        }
        let (x, y) = (
            ours["items"].as_array().unwrap(),
            want["items"].as_array().unwrap(),
        );
        for (p, q) in x.iter().zip(y) {
            if p != q {
                let (dx, dy) = (
                    p["definition"].as_str().unwrap(),
                    q["definition"].as_str().unwrap(),
                );
                let at = dx
                    .bytes()
                    .zip(dy.bytes())
                    .position(|(a, b)| a != b)
                    .unwrap_or(dx.len().min(dy.len()));
                let from = at.saturating_sub(60);
                failures.push(format!(
                    "{op}: item {} differs at byte {at}\n ours:   ...{}\n ArcGIS: ...{}",
                    p["name"],
                    &dx[from..(at + 160).min(dx.len())],
                    &dy[from..(at + 160).min(dy.len())]
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
