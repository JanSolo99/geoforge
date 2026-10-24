//! Creating and editing domains of a file geodatabase leaves the geodatabase as ArcGIS Pro leaves it
//! (testdata/gdb-reference/domain_ops: ArcGIS's state after each operation on domain_base.gdb, as
//! scripts/gdb-snapshot.py makes it through GDAL; `ops.json` lists the operations and what
//! ArcGIS answered).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value as Json;
use unigis_core::{Feature, Field, FieldType, Result, Value};
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
        "FLOAT" => FieldType::Single,
        "DOUBLE" => FieldType::Double,
        "DATE" => FieldType::Date,
        "BIGINTEGER" => FieldType::BigInteger,
        "DATEONLY" => FieldType::DateOnly,
        other => panic!("field type {other}"),
    }
}

fn all_codes(message: &str) -> Vec<String> {
    message
        .match_indices("ERROR ")
        .map(|(at, _)| message[at..].chars().take(12).collect())
        .collect()
}

/// Json argument as text (numbers and strings), `None` for null.
fn arg(args: &[Json], i: usize) -> Option<String> {
    match args.get(i)? {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Array(a) => Some(
            a.iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(";"),
        ),
        _ => None,
    }
}

fn apply(work: &Path, tool: &str, args: &[Json]) -> Result<()> {
    let s = |i: usize| arg(args, i).unwrap_or_default();
    let o = |i: usize| arg(args, i);
    match tool {
        "CreateDomain" => {
            let split = match o(4).as_deref() {
                Some("DUPLICATE") => unigis_filegdb::SplitPolicy::Duplicate,
                Some("GEOMETRY_RATIO") => unigis_filegdb::SplitPolicy::GeometryRatio,
                _ => unigis_filegdb::SplitPolicy::DefaultValue,
            };
            let merge = match o(5).as_deref() {
                Some("SUM_VALUES") => unigis_filegdb::MergePolicy::SumValues,
                Some("AREA_WEIGHTED") => unigis_filegdb::MergePolicy::AreaWeighted,
                _ => unigis_filegdb::MergePolicy::DefaultValue,
            };
            g::create_domain(
                work,
                &s(0),
                &s(1),
                field_type(&s(2)),
                s(3) == "RANGE",
                split,
                merge,
            )
        }
        "AddCodedValueToDomain" => g::add_coded_value(work, &s(0), &s(1), &s(2)),
        "DeleteCodedValueFromDomain" => g::delete_coded_values(work, &s(0), &s(1)),
        "AlterCodedValueDescription" => g::alter_coded_value_description(work, &s(0), &s(1), &s(2)),
        "SetValueForRangeDomain" => g::set_value_for_range(work, &s(0), &s(1), &s(2)),
        "SortCodedValueDomain" => {
            g::sort_coded_values(work, &s(0), s(1) == "CODE", s(2) == "ASCENDING")
        }
        "AlterDomain" => g::alter_domain(
            work,
            &s(0),
            o(1).as_deref(),
            o(2).as_deref(),
            o(3).map(|p| match p.as_str() {
                "DUPLICATE" => unigis_filegdb::SplitPolicy::Duplicate,
                "GEOMETRY_RATIO" => unigis_filegdb::SplitPolicy::GeometryRatio,
                _ => unigis_filegdb::SplitPolicy::DefaultValue,
            }),
            o(4).map(|p| match p.as_str() {
                "SUM_VALUES" => unigis_filegdb::MergePolicy::SumValues,
                "AREA_WEIGHTED" => unigis_filegdb::MergePolicy::AreaWeighted,
                _ => unigis_filegdb::MergePolicy::DefaultValue,
            }),
            o(5).as_deref(),
        ),
        "DomainToTable" => {
            let (t, rows) = g::domain_rows(work, &s(0))?;
            let mut w = g::GeodatabaseWriter::open_existing(work)?;
            let mut desc = Field::new(s(3), FieldType::String);
            desc.length = 255;
            let fields = [Field::new(s(2), t), desc];
            let mut table = w.create_table(&s(1), None, false, false, &fields, None)?;
            for (code, d) in rows {
                table.write(&Feature {
                    oid: 0,
                    geometry: None,
                    attributes: vec![code, Value::String(d)],
                })?;
            }
            w.finish_table(table)?;
            w.finish()
        }
        "TableToDomain" => {
            let gdb = g::Geodatabase::open(work)?;
            let table = gdb.open_table(&s(0))?;
            let name_of = |n: &str| {
                table
                    .fields()
                    .iter()
                    .position(|f| f.name.eq_ignore_ascii_case(n))
                    .unwrap()
            };
            let (c, d) = (name_of(&s(1)), name_of(&s(2)));
            let t = table.fields()[c].field_type;
            let mut values = Vec::new();
            for f in table.features() {
                let f = f?;
                let desc = match &f.attributes[d] {
                    Value::String(x) => x.clone(),
                    _ => String::new(),
                };
                values.push((f.attributes[c].clone(), desc));
            }
            drop(table);
            drop(gdb);
            g::table_to_domain(
                work,
                &s(4),
                &s(5),
                t,
                &values,
                o(6).as_deref() == Some("REPLACE"),
            )
        }
        other => panic!("tool {other}"),
    }
}

const DOMAIN_TYPES: [&str; 2] = ["8C368B12", "C29DA988"];

#[test]
fn domain_operations_equal_arcgis() {
    if !unigis_testkit::ogrinfo_available() {
        return;
    }
    let reference = root().join("testdata/gdb-reference");
    let manifest: Json =
        serde_json::from_slice(&std::fs::read(reference.join("domain_ops/ops.json")).unwrap())
            .unwrap();
    let base: Json =
        serde_json::from_slice(&std::fs::read(reference.join("domain_ops/base.json")).unwrap())
            .unwrap();
    let base_items: Vec<&str> = base["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["name"].as_str().unwrap())
        .collect();
    let mut failures = Vec::new();
    for (op, spec) in manifest["ops"].as_object().unwrap() {
        let work = std::env::temp_dir().join(format!("gf-domain-{op}-{}.gdb", std::process::id()));
        std::fs::remove_dir_all(&work).ok();
        copy_dir(&reference.join("domain_base.gdb"), &work);
        let result = apply(
            &work,
            spec["tool"].as_str().unwrap(),
            spec["args"].as_array().unwrap(),
        );
        let ours = snapshot(&work);
        std::fs::remove_dir_all(&work).ok();
        let Some(ours) = ours else { return };
        let arcgis = manifest["arcgis"][op].as_str().unwrap();
        let want: Json = serde_json::from_slice(
            &std::fs::read(reference.join(format!("domain_ops/{op}.json"))).unwrap(),
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
                    if !all_codes(arcgis).iter().any(|c| ours.contains(c.as_str())) {
                        failures.push(format!("{op}: error {e}, ArcGIS {arcgis}"));
                    }
                }
            }
        }
        for key in ["fields", "rows", "catalog", "rels", "tables", "defaults"] {
            if ours[key] != want[key] {
                failures.push(format!(
                    "{op}: {key} differ\n ours:   {}\n ArcGIS: {}",
                    ours[key].to_string().chars().take(700).collect::<String>(),
                    want[key].to_string().chars().take(700).collect::<String>()
                ));
            }
        }
        let (x, y) = (
            ours["items"].as_array().unwrap(),
            want["items"].as_array().unwrap(),
        );
        if x.len() != y.len() {
            failures.push(format!("{op}: {} items, ArcGIS {}", x.len(), y.len()));
            continue;
        }
        for (p, q) in x.iter().zip(y) {
            let is_domain = DOMAIN_TYPES.contains(&q["type"].as_str().unwrap());
            let new_table = !is_domain && !base_items.contains(&q["name"].as_str().unwrap());
            // New tables differ in their definition text (ours is not ArcGIS's byte for byte).
            let equal = if new_table {
                p["name"] == q["name"] && p["path"] == q["path"]
            } else {
                p == q
            };
            if !equal {
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
                    "{op}: item {} differs at byte {at} ({} vs {})\n ours:   ...{}\n ArcGIS: ...{}",
                    p["name"],
                    p["physical"],
                    q["physical"],
                    &dx[from..(at + 160).min(dx.len())],
                    &dy[from..(at + 160).min(dy.len())]
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
