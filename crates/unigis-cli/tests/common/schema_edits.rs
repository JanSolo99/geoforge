//! Portable ArcGIS snapshots for geodatabase-only, in-place schema edits.
use serde_json::{Value as Json, json};
use std::path::Path;
use std::process::Command;
use unigis_core::{FeatureClass, FieldType, Value};
use unigis_filegdb::{DomainKind, Geodatabase};

fn number(v: &Value) -> f64 {
    match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::Double(n) => *n,
        _ => panic!("unexpected range value {v:?}"),
    }
}
fn code(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        _ => panic!("unexpected code {v:?}"),
    }
}

pub fn compare(case: &str, info: &Json, root: &Path, out_dir: &Path) -> Vec<String> {
    let source = root.join(info["run"]["params"]["input"].as_str().unwrap());
    let dataset = source.file_name().unwrap();
    let source_gdb = source.parent().unwrap();
    let copied = out_dir.join(format!("{case}.gdb"));
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
    let target = copied.join(dataset);
    let before_gdb = Geodatabase::open(&copied).unwrap();
    let before = before_gdb.open_table(dataset.to_str().unwrap()).unwrap();
    let old_fields = before.fields().to_vec();
    let table_base = before_gdb
        .table_base(dataset.to_str().unwrap())
        .unwrap()
        .file_name()
        .unwrap()
        .to_owned();
    let old_indexes: Vec<_> = before
        .indexes()
        .iter()
        .map(|i| (i.name.clone(), i.expression.clone()))
        .collect();
    let rows = before.features().collect::<Result<Vec<_>, _>>().unwrap();
    let wkt = before.spatial_reference_wkt().map(str::to_owned);
    let flags = (before.geometry_kind(), before.has_z(), before.has_m());
    drop(before);
    drop(before_gdb);
    let run = || {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ug"));
        cmd.current_dir(root)
            .args(["run", info["run"]["tool"].as_str().unwrap()]);
        for (name, value) in info["run"]["params"].as_object().unwrap() {
            cmd.arg(format!(
                "{name}={}",
                if name == "input" {
                    target.display().to_string()
                } else {
                    value.as_str().unwrap().to_owned()
                }
            ));
        }
        cmd.output().unwrap()
    };
    let mut diffs = Vec::new();
    let result = run();
    if info["status"] == "failed" {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let matched = info["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|m| m.as_str()?.strip_prefix("ERROR ")?.get(..6))
            .any(|c| stderr.contains(c));
        if result.status.success() || !matched {
            diffs.push(format!("expected refusal, got {result:?}"));
        }
        std::fs::remove_dir_all(copied).unwrap();
        return diffs;
    }
    if !result.status.success() {
        diffs.push(format!(
            "schema edit failed: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    if diffs.is_empty() {
        let gdb = Geodatabase::open(&copied).unwrap();
        let table = gdb.open_table(dataset.to_str().unwrap()).unwrap();
        if gdb
            .table_base(dataset.to_str().unwrap())
            .unwrap()
            .file_name()
            .unwrap()
            != table_base
            || table
                .indexes()
                .iter()
                .map(|i| (i.name.clone(), i.expression.clone()))
                .collect::<Vec<_>>()
                != old_indexes
        {
            diffs.push("table identity or indexes changed".into());
        }
        if table
            .fields()
            .iter()
            .skip(old_fields.len())
            .any(|f| f.default.is_some())
        {
            diffs.push("metadata fields have unexpected defaults".into());
        }
        let ours: Vec<_> = table
            .fields()
            .iter()
            .map(|f| {
                json!({
                    "name": f.name, "aliasName": f.alias.as_deref().unwrap_or(&f.name),
                    "type":format!("{:?}",f.field_type), "length":match f.field_type {
                        FieldType::SmallInteger=>2, FieldType::Integer|FieldType::Single=>4,
                        FieldType::Double|FieldType::Date=>8, _=>f.length },
                    "isNullable":f.nullable,"domain":f.domain.as_deref().unwrap_or(""),
                })
            })
            .collect();
        let expected:Vec<_>=info["schema"]["fields"].as_array().unwrap().iter().filter(|f| !matches!(f["type"].as_str(),Some("OID"|"Geometry"))).map(|f| json!({
            "name":f["name"],"aliasName":f["aliasName"],"type":f["type"],"length":f["length"],"isNullable":f["isNullable"],"domain":f["domain"],
        })).collect();
        if ours != expected {
            diffs.push(format!(
                "field schema differs\nours: {ours:?}\nArcGIS: {expected:?}"
            ));
        }
        if let Some(cogo) = info.get("cogo") {
            let items = unigis_filegdb::Table::open(copied.join("a00000004")).unwrap();
            let name_col = items
                .fields()
                .iter()
                .position(|f| f.name == "Name")
                .unwrap();
            let def_col = items
                .fields()
                .iter()
                .position(|f| f.name == "Definition")
                .unwrap();
            let row=items.features().map(Result::unwrap).find(|f|matches!(&f.attributes[name_col],Value::String(n) if n==dataset.to_str().unwrap())).unwrap();
            let xml = match &row.attributes[def_col] {
                Value::Xml(x) | Value::String(x) => x,
                _ => panic!("missing definition"),
            };
            let doc = roxmltree::Document::parse(xml).unwrap();
            let mut required = serde_json::Map::new();
            for name in cogo["required"].as_object().unwrap().keys() {
                let flag = doc
                    .descendants()
                    .filter(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoEx")
                    .find(|n| {
                        n.children().any(|c| {
                            c.is_element()
                                && c.tag_name().name() == "Name"
                                && c.text() == Some(name)
                        })
                    })
                    .is_some_and(|n| {
                        n.children().any(|c| {
                            c.is_element()
                                && c.tag_name().name() == "Required"
                                && c.text() == Some("true")
                        })
                    });
                required.insert(name.clone(), json!(flag));
            }
            if Json::Object(required) != cogo["required"] {
                diffs.push("COGO required field markers differ".into());
            }
        }
        if let Some(tracking) = info.get("editor_tracking") {
            let items = unigis_filegdb::Table::open(copied.join("a00000004")).unwrap();
            let column = |name: &str| items.fields().iter().position(|f| f.name == name).unwrap();
            let row = items.features().map(Result::unwrap).find(|r| matches!(&r.attributes[column("Name")], Value::String(n) if n == dataset.to_str().unwrap())).unwrap();
            let xml = match &row.attributes[column("Definition")] {
                Value::Xml(x) | Value::String(x) => x,
                _ => panic!("missing definition"),
            };
            let doc = roxmltree::Document::parse(xml).unwrap();
            let root = doc.root_element();
            let text = |tag: &str| {
                root.children()
                    .find(|n| n.is_element() && n.tag_name().name() == tag)
                    .and_then(|n| n.text())
                    .unwrap_or("")
            };
            let properties = json!({
                "editorTrackingEnabled":text("EditorTrackingEnabled") == "true",
                "creatorFieldName":text("CreatorFieldName"),
                "createdAtFieldName":text("CreatedAtFieldName"),
                "editorFieldName":text("EditorFieldName"),
                "editedAtFieldName":text("EditedAtFieldName"),
                "isTimeInUTC":text("IsTimeInUTC") == "true",
            });
            let mut editable = serde_json::Map::new();
            for name in tracking["editable"].as_object().unwrap().keys() {
                let entry = doc
                    .descendants()
                    .filter(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoEx")
                    .find(|n| {
                        n.children().any(|c| {
                            c.is_element()
                                && c.tag_name().name() == "Name"
                                && c.text() == Some(name)
                        })
                    });
                let flag = !entry.is_some_and(|n| {
                    n.children().any(|c| {
                        c.is_element()
                            && c.tag_name().name() == "Editable"
                            && c.text() == Some("false")
                    })
                });
                if table.field_editable(name) != Some(flag) {
                    diffs.push(format!(
                        "physical field editability disagrees with catalog: {name}"
                    ));
                }
                editable.insert(name.clone(), json!(flag));
            }
            if properties != tracking["properties"]
                || Json::Object(editable) != tracking["editable"]
            {
                diffs.push(format!(
                    "editor tracking metadata differs: {properties}; expected {tracking}"
                ));
            }
        }
        let mut domains:Vec<_>=gdb.domains().iter().map(|d|json!({
            "name":d.name,"description":d.description,"type":match d.field_type {FieldType::SmallInteger=>"Short",FieldType::String=>"Text",_=>panic!("unexpected domain type")},
            "split":format!("{:?}",d.split_policy),"merge":format!("{:?}",d.merge_policy),
            "kind":match d.kind {DomainKind::Range{..}=>"Range",_=>"CodedValue"},
            "values":match &d.kind {
                DomainKind::Range{min,max}=>json!([number(min),number(max)]),
                DomainKind::Coded(values)=>Json::Object(values.iter().map(|(v,s)|(code(v),json!(s))).collect()),
            },
        })).collect();
        let mut expected = info["domains"].as_array().unwrap().clone();
        domains.sort_by_key(|d| d["name"].as_str().unwrap().to_owned());
        expected.sort_by_key(|d| d["name"].as_str().unwrap().to_owned());
        if domains != expected {
            diffs.push(format!(
                "domains differ: ours {domains:?}; ArcGIS {expected:?}"
            ));
        }
        if flags != (table.geometry_kind(), table.has_z(), table.has_m())
            || wkt.as_deref() != table.spatial_reference_wkt()
        {
            diffs.push("geometry flags or spatial reference changed".into());
        }
        let after = table.features().collect::<Result<Vec<_>, _>>().unwrap();
        if rows.len() != after.len() {
            diffs.push("row count changed".into());
        }
        for (old, new) in rows.iter().zip(&after) {
            if old.oid != new.oid
                || old.geometry != new.geometry
                || old.attributes != new.attributes[..old_fields.len()]
                || new.attributes[old_fields.len()..]
                    .iter()
                    .any(|v| !v.is_null())
            {
                diffs.push(format!(
                    "source row {} changed or new fields not NULL",
                    old.oid
                ));
            }
        }
        let mut preserved_fields = old_fields.clone();
        if info["tool"] == "management.EnableCOGO" {
            // Public EnableCOGO resets existing dimension aliases; every other
            // field property and all existing values must remain unchanged.
            for field in &mut preserved_fields {
                if let Some((_, alias)) = [
                    ("Direction", "Direction"),
                    ("Distance", "Distance"),
                    ("Radius", "Radius"),
                    ("ArcLength", "Arc Length"),
                    ("Radius2", "Radius2"),
                ]
                .iter()
                .find(|(name, _)| field.name.eq_ignore_ascii_case(name))
                {
                    field.alias = (*alias != field.name).then(|| alias.to_string());
                }
            }
        }
        if table.fields()[..old_fields.len()] != preserved_fields {
            diffs.push("existing field definition changed".into());
        }
        let first_fields = table.fields().to_vec();
        let first_domains = gdb.domains().to_vec();
        drop(table);
        drop(gdb);
        let repeat = run();
        let again = Geodatabase::open(&copied).unwrap();
        if !repeat.status.success()
            || again.domains() != first_domains
            || again
                .open_table(dataset.to_str().unwrap())
                .unwrap()
                .fields()
                != first_fields
        {
            diffs.push("repeated schema edit changed the result or failed".into());
        }
    }
    std::fs::remove_dir_all(copied).unwrap();
    diffs
}
