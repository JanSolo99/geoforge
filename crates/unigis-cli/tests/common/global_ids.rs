//! AddGlobalIDs contract: random identifiers, exact schema and stable source records.
use serde_json::Value as Json;
use std::{collections::HashSet, path::Path, process::Command};
use unigis_core::{FieldType, Value};
use unigis_filegdb::Geodatabase;

pub fn compare(case: &str, info: &Json, root: &Path, out: &Path) -> Vec<String> {
    let inputs = info["run"]["params"]["in_datasets"].as_str().unwrap();
    let source = root.join(inputs.split(';').next().unwrap());
    let source_gdb = source.parent().unwrap();
    let copied = out.join(format!("{case}.gdb"));
    if copied.exists() {
        std::fs::remove_dir_all(&copied).unwrap();
    }
    std::fs::create_dir(&copied).unwrap();
    for e in std::fs::read_dir(source_gdb).unwrap() {
        let e = e.unwrap();
        if !e.file_name().to_string_lossy().ends_with(".lock") {
            std::fs::copy(e.path(), copied.join(e.file_name())).unwrap();
        }
    }
    let arg = inputs
        .split(';')
        .map(|p| {
            copied
                .join(Path::new(p).file_name().unwrap())
                .display()
                .to_string()
        })
        .collect::<Vec<_>>()
        .join(";");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_ug"))
            .args(["run", "add_global_ids", &format!("in_datasets={arg}")])
            .output()
            .unwrap()
    };
    let result = run();
    let mut diffs = Vec::new();
    if info["status"] == "failed" {
        if result.status.success() || !String::from_utf8_lossy(&result.stderr).contains("000242") {
            diffs.push(format!("expected000242: {result:?}"));
        }
        return diffs;
    }
    if !result.status.success() {
        return vec![format!("edit failed: {result:?}")];
    }
    let before = Geodatabase::open(source_gdb).unwrap();
    let after = Geodatabase::open(&copied).unwrap();
    let mut all_ids = HashSet::new();
    let mut saved = Vec::new();
    for (name, expected) in info["targets"].as_object().unwrap() {
        let old = before.open_table(name).unwrap();
        let new = after.open_table(name).unwrap();
        let old_rows = old.features().collect::<Result<Vec<_>, _>>().unwrap();
        let rows = new.features().collect::<Result<Vec<_>, _>>().unwrap();
        let gid = expected["globalIDFieldName"].as_str().unwrap();
        let fields = &expected["fields"].as_array().unwrap();
        assert_eq!(new.all_fields().len(), fields.len(), "{case}");
        for (f, e) in new.all_fields().iter().zip(fields.iter()) {
            let ty = match f.field_type {
                unigis_filegdb::GdbFieldType::GlobalId => "GlobalID".to_owned(),
                unigis_filegdb::GdbFieldType::Int32 => "Integer".to_owned(),
                unigis_filegdb::GdbFieldType::ObjectId => "OID".to_owned(),
                other => format!("{other:?}"),
            };
            if f.name != e["name"].as_str().unwrap()
                || ty != e["type"].as_str().unwrap()
                || f.nullable != e["nullable"].as_bool().unwrap()
                || new.field_editable(&f.name) != e["editable"].as_bool()
            {
                diffs.push(format!("{name}: field schema differs {f:?} vs {e}"));
            }
            if f.name == gid
                && (f.length != 38 || f.field_type != unigis_filegdb::GdbFieldType::GlobalId)
            {
                diffs.push("bad GlobalID storage".into());
            }
        }
        let indexes = new
            .indexes()
            .iter()
            .map(|i| (i.name.clone(), i.expression.clone()))
            .collect::<Vec<_>>();
        let expected_indexes = expected["indexes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| {
                (
                    i["name"].as_str().unwrap().to_owned(),
                    i["fields"][0].as_str().unwrap().to_owned(),
                )
            })
            .collect::<Vec<_>>();
        if indexes != expected_indexes {
            diffs.push(format!(
                "{name}: indexes {indexes:?} vs {expected_indexes:?}"
            ));
        }
        let catalog = unigis_filegdb::Table::open(copied.join("a00000004")).unwrap();
        let ni = catalog
            .fields()
            .iter()
            .position(|f| f.name == "Name")
            .unwrap();
        let di = catalog
            .fields()
            .iter()
            .position(|f| f.name == "Definition")
            .unwrap();
        let row = catalog
            .features()
            .map(|r| r.unwrap())
            .find(|r| r.attributes[ni] == Value::String(name.clone()))
            .unwrap();
        let xml = match &row.attributes[di] {
            Value::Xml(s) | Value::String(s) => s,
            _ => panic!("definition type"),
        };
        let doc = roxmltree::Document::parse(xml).unwrap();
        let text = |tag: &str| {
            doc.root_element()
                .children()
                .find(|n| n.has_tag_name(tag))
                .and_then(|n| n.text())
        };
        if text("HasGlobalID") != Some("true") || text("GlobalIDFieldName") != Some(gid) {
            diffs.push("GlobalID catalog identity differs".into());
        }
        let required = doc
            .descendants()
            .filter(|n| n.has_tag_name("GPFieldInfoEx"))
            .any(|n| {
                n.children()
                    .any(|c| c.has_tag_name("Name") && c.text() == Some(gid))
                    && n.children()
                        .any(|c| c.has_tag_name("Required") && c.text() == Some("true"))
            });
        if !required {
            diffs.push("GlobalID missing required catalog flag".into());
        }
        let gid_i = new
            .fields()
            .iter()
            .position(|f| f.field_type == FieldType::GlobalId)
            .unwrap();
        if rows.len() != old_rows.len() {
            diffs.push("row count changed".into());
        }
        for (row, old_row) in rows.iter().zip(&old_rows) {
            let mut stripped = row.clone();
            let id = stripped.attributes.remove(gid_i);
            if stripped != *old_row {
                diffs.push("source row changed".into());
            }
            let Value::Guid(id) = id else {
                panic!("GlobalID not GUID");
            };
            let clean = id.trim_matches(['{', '}']);
            if clean.len() != 36
                || clean.chars().nth(14) != Some('4')
                || !all_ids.insert(id.clone())
                || !clean.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
            {
                diffs.push(format!("invalid/duplicate UUID4: {id}"));
            }
        }
        saved.push((name.clone(), rows, new.fields().to_vec()));
    }
    drop(after);
    if !run().status.success() {
        diffs.push("repeat failed".into());
    }
    let repeated = Geodatabase::open(&copied).unwrap();
    for (name, rows, fields) in saved {
        let t = repeated.open_table(&name).unwrap();
        if t.features().collect::<Result<Vec<_>, _>>().unwrap() != rows || t.fields() != fields {
            diffs.push("repeat changed IDs/schema".into());
        }
    }
    diffs
}
