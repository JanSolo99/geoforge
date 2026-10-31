//! Geodatabase scenarios: the steps ArcGIS ran (`ug run` with the same parameters) in a new
//! folder, each with ArcGIS's outcome, then the geodatabase's schema as ArcGIS described it.
use serde_json::{Value as Json, json};
use std::path::Path;
use std::process::Command;
use unigis_core::{FieldType, Value};
use unigis_filegdb::{DomainKind, Geodatabase};

fn type_name(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "SmallInteger",
        FieldType::Integer => "Integer",
        FieldType::BigInteger => "BigInteger",
        FieldType::Single => "Single",
        FieldType::Double => "Double",
        FieldType::String => "String",
        FieldType::Date => "Date",
        FieldType::DateOnly => "DateOnly",
        FieldType::TimeOnly => "TimeOnly",
        FieldType::TimestampOffset => "TimestampOffset",
        FieldType::Blob => "Blob",
        FieldType::Guid => "Guid",
        FieldType::GlobalId => "GlobalID",
        FieldType::ObjectId => "OID",
        FieldType::Geometry => "Geometry",
        _ => "?",
    }
}

fn length(t: FieldType, length: u32) -> u32 {
    match t {
        FieldType::SmallInteger => 2,
        FieldType::Integer | FieldType::Single => 4,
        FieldType::Double
        | FieldType::Date
        | FieldType::BigInteger
        | FieldType::DateOnly
        | FieldType::TimeOnly => 8,
        FieldType::Guid | FieldType::GlobalId => 38,
        _ => length,
    }
}

fn value(v: &Value) -> Json {
    match v {
        Value::Null => Json::Null,
        Value::SmallInteger(n) => json!(n),
        Value::Integer(n) => json!(n),
        Value::BigInteger(n) => json!(n),
        Value::Single(x) => json!(x),
        Value::Double(x) => json!(x),
        Value::String(s) => json!(s),
        other => json!(format!("{other:?}")),
    }
}

/// Python's `str()` of a domain value: codes as they are, range limits as floats.
fn text(v: &Value, float: bool) -> String {
    let n = match v {
        Value::SmallInteger(n) => f64::from(*n),
        Value::Integer(n) => f64::from(*n),
        Value::BigInteger(n) => *n as f64,
        Value::Single(x) => f64::from(*x),
        Value::Double(x) => *x,
        Value::String(s) => return s.clone(),
        other => return format!("{other:?}"),
    };
    if float || n.fract() != 0.0 {
        format!("{n:?}")
    } else {
        format!("{n}")
    }
}

fn domain_type(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "Short",
        FieldType::Integer => "Long",
        FieldType::BigInteger => "BigInteger",
        FieldType::Single => "Float",
        FieldType::Double => "Double",
        FieldType::String => "Text",
        FieldType::Date => "Date",
        _ => "?",
    }
}

/// Our geodatabase described the way the generator's `gdb_snapshot` describes ArcGIS's.
fn snapshot(path: &Path) -> Json {
    let gdb = Geodatabase::open(path).unwrap();
    let mut items: Vec<String> = gdb
        .entries()
        .iter()
        .map(|e| match &e.feature_dataset {
            Some(fds) => format!("{fds}/{}", e.name),
            None => e.name.clone(),
        })
        .collect();
    items.extend(gdb.feature_datasets().iter().map(|f| format!("{f}/")));
    items.extend(gdb.relationship_classes().iter().map(|r| r.name.clone()));
    items.sort();
    let mut tables = serde_json::Map::new();
    for e in gdb.entries() {
        let t = gdb.open_table(&e.name).unwrap();
        let mut fields = vec![
            json!({"name": t.oid_field_name().unwrap_or("OBJECTID"), "type": "OID",
            "length": 4, "alias": t.oid_field_name().unwrap_or("OBJECTID"), "domain": "",
            "default": null, "nullable": false}),
        ];
        if let Some(shape) = t.shape_field() {
            fields.push(json!({"name": shape.name, "type": "Geometry", "length": 0,
                "alias": shape.name, "domain": "", "default": null, "nullable": true}));
        }
        fields.extend(t.fields().iter().map(|f| {
            json!({"name": f.name, "type": type_name(f.field_type),
                "length": length(f.field_type, f.length),
                "alias": f.alias.as_deref().unwrap_or(&f.name),
                "domain": f.domain.as_deref().unwrap_or(""),
                "default": f.default.as_ref().map_or(Json::Null, value),
                "nullable": f.nullable})
        }));
        let mut indexes: Vec<Json> = t
            .indexes()
            .iter()
            .filter(|i| !i.name.to_uppercase().starts_with("FDO_"))
            .map(|i| json!([i.name, [i.expression]]))
            .collect();
        indexes.sort_by_key(|i| i.to_string());
        let subtypes: Vec<Json> = t.subtypes().map_or_else(Vec::new, |s| {
            let mut v: Vec<_> = s
                .subtypes
                .iter()
                .map(|st| {
                    json!({"code": st.code, "name": st.name,
                        "default": st.code == s.default_code, "field": s.field})
                })
                .collect();
            v.sort_by_key(|j| j["code"].as_i64());
            v
        });
        tables.insert(
            e.name.clone(),
            json!({"fields": fields, "indexes": indexes, "subtypes": subtypes}),
        );
    }
    let mut domains: Vec<Json> = gdb
        .domains()
        .iter()
        .map(|d| {
            let (kind, values) = match &d.kind {
                DomainKind::Coded(values) => (
                    "CodedValue",
                    values
                        .iter()
                        .map(|(c, s)| json!([text(c, false), s]))
                        .collect::<Vec<_>>(),
                ),
                DomainKind::Range { min, max } => (
                    "Range",
                    vec![json!(text(min, true)), json!(text(max, true))],
                ),
            };
            json!({"name": d.name, "description": d.description, "type": domain_type(d.field_type),
                "kind": kind, "split": format!("{:?}", d.split_policy),
                "merge": format!("{:?}", d.merge_policy), "values": values})
        })
        .collect();
    domains.sort_by_key(|d| d["name"].as_str().map(str::to_owned));
    let relationships: Vec<Json> = gdb
        .relationship_classes()
        .iter()
        .map(|r| {
            let mut keys = vec![
                json!([r.origin_primary_key, "OriginPrimary"]),
                json!([r.origin_foreign_key, "OriginForeign"]),
            ];
            if let (Some(p), Some(f)) = (&r.destination_primary_key, &r.destination_foreign_key) {
                keys.push(json!([p, "DestinationPrimary"]));
                keys.push(json!([f, "DestinationForeign"]));
            }
            json!({"name": r.name, "cardinality": format!("{:?}", r.cardinality),
                "origin": [r.origin], "destination": [r.destination],
                "forward": r.forward_label, "backward": r.backward_label,
                "composite": r.is_composite, "notification": r.notification, "keys": keys})
        })
        .collect();
    json!({"items": items, "tables": tables, "domains": domains, "relationships": relationships})
}

pub fn compare(case: &str, info: &Json, out_dir: &Path) -> Vec<String> {
    let scenario = &info["gdb_scenario"];
    let ws = out_dir.join(format!("{case}-ws"));
    if ws.exists() {
        std::fs::remove_dir_all(&ws).unwrap();
    }
    std::fs::create_dir_all(&ws).unwrap();
    let place = |s: &str| s.replace("<ws>", &ws.display().to_string());
    let mut diffs = Vec::new();
    for (k, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        if step["tool"] == "seed" {
            // An ArcGIS-made geodatabase as the starting point.
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(step["run"]["params"]["source"].as_str().unwrap());
            copy_dir(&source, &ws.join("t.gdb"));
            continue;
        }
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_ug"));
        cmd.args(["run", step["run"]["tool"].as_str().unwrap()]);
        for (name, v) in step["run"]["params"].as_object().unwrap() {
            cmd.arg(format!("{name}={}", place(v.as_str().unwrap())));
        }
        let result = cmd.output().unwrap();
        let label = format!("step {k} ({})", step["tool"].as_str().unwrap_or(""));
        if step["status"] == "failed" {
            let stderr = String::from_utf8_lossy(&result.stderr);
            let codes: Vec<&str> = step["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|m| m.as_str()?.strip_prefix("ERROR ")?.get(..6))
                .collect();
            if result.status.success() || !codes.iter().any(|c| stderr.contains(c)) {
                diffs.push(format!(
                    "{label}: expected ArcGIS's ERROR {codes:?}, got {}",
                    if result.status.success() {
                        "success".into()
                    } else {
                        stderr.into_owned()
                    }
                ));
            }
        } else if !result.status.success() {
            diffs.push(format!(
                "{label} failed: {}",
                String::from_utf8_lossy(&result.stderr).trim()
            ));
        }
    }
    let ours = snapshot(&ws.join("t.gdb"));
    let expected = &scenario["snapshot"];
    for key in ["items", "tables", "domains", "relationships"] {
        if key == "tables" {
            for (name, table) in expected["tables"].as_object().unwrap() {
                if ours["tables"][name] != *table {
                    diffs.push(format!(
                        "table {name}:\n      ours   {}\n      ArcGIS {table}",
                        ours["tables"][name]
                    ));
                }
            }
        } else if ours[key] != expected[key] {
            diffs.push(format!(
                "{key}:\n      ours   {}\n      ArcGIS {}",
                ours[key], expected[key]
            ));
        }
    }
    std::fs::remove_dir_all(ws).unwrap();
    diffs
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}
