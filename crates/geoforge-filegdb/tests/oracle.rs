//! Compares the FileGDB reader with GDAL's OpenFileGDB driver on every `.gdb` in
//! `testdata/large/filegdb` (from `scripts/fetch-testdata.sh`).

use std::path::{Path, PathBuf};

use geoforge_core::curves::DEFAULT_MAX_ANGLE_STEP;
use geoforge_core::{FieldType, Value};
use geoforge_filegdb::{Cardinality, DomainKind, Geodatabase, MergePolicy, SplitPolicy, Table};
use geoforge_testkit::{
    compare_feature, compare_shapes, gdal_version, ogr_dataset_json, ogr_layer, ogr_layer_names,
    ogr_linearized, ogrinfo_available,
};
use serde_json::Value as Json;

fn geodatabases(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.join("a00000001.gdbtable").is_file()
                && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("gdb"))
            {
                out.push(p);
            } else {
                out.extend(geodatabases(&p));
            }
        }
    }
    out.sort();
    out
}

/// Tables we deliberately do not read yet.
const UNSUPPORTED: &[(&str, &str)] = &[("Lake_labels", "compressed .cdf table")];

/// Curves whose stored definition is inconsistent, where GDAL and we each improvise
/// (here: a circular arc whose center is 1 unit from the start but 2 from the end).
const MALFORMED_CURVES: &[(&str, i64)] = &[("test", 6)];

/// Sparse 64-bit OBJECTID tables: GDAL warns that its ObjectIDs "will not be
/// accurate", so features are matched in order instead of by OBJECTID.
fn gdal_oids_unreliable(gdb: &Path) -> bool {
    gdb.ends_with("objectid64/with_holes_8.gdb")
}

fn diff_table(gdb: &Path, name: &str, table: &Table) -> Result<Vec<String>, String> {
    let gdal = ogr_layer(gdb, Some(name))?;
    let names: Vec<&str> = table.fields().iter().map(|f| f.name.as_str()).collect();
    let mut diffs = Vec::new();
    let gdal_names: Vec<&str> = gdal.fields.iter().map(|f| f.name.as_str()).collect();
    if names != gdal_names {
        diffs.push(format!("fields: ours {names:?}, GDAL {gdal_names:?}"));
    }
    let mut count = 0;
    let by_order: Vec<_> = gdal.features.values().collect();
    let mut linearized = None;
    for feature in table.features() {
        let feature = match feature {
            Ok(f) => f,
            Err(e) => {
                diffs.push(format!("read error: {e}"));
                continue;
            }
        };
        count += 1;
        let gdal_feature = if gdal_oids_unreliable(gdb) {
            by_order.get(count - 1).copied()
        } else {
            gdal.features.get(&feature.oid)
        };
        // Curves: compare our densified shape with GDAL's linearization.
        let malformed = MALFORMED_CURVES.contains(&(name, feature.oid));
        if let Some(g) = feature
            .geometry
            .as_ref()
            .filter(|g| g.has_curves() && !malformed)
        {
            let linear =
                linearized.get_or_insert_with(|| ogr_linearized(gdb, name).unwrap_or_default());
            let ours = g
                .densify_curves(DEFAULT_MAX_ANGLE_STEP)
                .map_err(|e| e.to_string())?;
            let env = ours.envelope();
            let tol = 1e-3 * (env.xmax - env.xmin).hypot(env.ymax - env.ymin).max(1e-9);
            match linear.get(&feature.oid).cloned().flatten() {
                Some(wkt) => {
                    if let Some(d) = compare_shapes(&ours, &wkt, tol) {
                        diffs.push(format!("OBJECTID {} curve: {d}", feature.oid));
                    }
                }
                None => diffs.push(format!(
                    "OBJECTID {}: GDAL has no linearized geometry",
                    feature.oid
                )),
            }
        }
        match gdal_feature {
            Some(g) => diffs.extend(compare_feature(&feature, &names, g)),
            None => diffs.push(format!("OBJECTID {} not reported by GDAL", feature.oid)),
        }
    }
    if count != gdal.features.len() {
        diffs.push(format!(
            "feature count: ours {count}, GDAL {}",
            gdal.features.len()
        ));
    }
    Ok(diffs)
}

#[test]
fn filegdb_matches_gdal() {
    if !ogrinfo_available() {
        eprintln!("skipping: ogrinfo not installed");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb");
    let gdbs = geodatabases(&root);
    // ArcGIS Pro 3.2 tables (64-bit OIDs, new field types) need GDAL >= 3.9.
    let gdal_reads_pro_3_2 = gdal_version().is_some_and(|v| v >= (3, 9));
    let (mut tables, mut failures) = (0, 0);
    for path in &gdbs {
        let label = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let gdb = match Geodatabase::open(path) {
            Ok(g) => g,
            Err(e) => {
                failures += 1;
                eprintln!("✗ {label}: cannot open: {e}");
                continue;
            }
        };
        if let Ok(gdal_names) = ogr_layer_names(path) {
            let ours: Vec<&str> = gdb.entries().iter().map(|e| e.name.as_str()).collect();
            let missing: Vec<_> = gdal_names
                .iter()
                .filter(|n| !ours.contains(&n.as_str()))
                .collect();
            if !missing.is_empty() {
                failures += 1;
                eprintln!("✗ {label}: layers GDAL lists but we do not: {missing:?}");
            }
        }
        for entry in gdb.entries() {
            tables += 1;
            if let Some((_, why)) = UNSUPPORTED.iter().find(|(n, _)| *n == entry.name) {
                eprintln!("~ {label} / {}: skipped ({why})", entry.name);
                continue;
            }
            let diffs = match gdb.open_table(&entry.name) {
                Ok(t) if t.requires_pro_3_2() && !gdal_reads_pro_3_2 => {
                    eprintln!(
                        "~ {label} / {}: skipped (GDAL too old for Pro 3.2 tables)",
                        entry.name
                    );
                    continue;
                }
                Ok(t) => diff_table(path, &entry.name, &t).unwrap_or_else(|e| vec![e]),
                Err(e) => vec![format!("cannot open: {e}")],
            };
            // GDAL before 3.9 reads some newer tables (the utility network ones) as
            // having no fields at all: nothing to compare.
            if !gdal_reads_pro_3_2
                && diffs
                    .iter()
                    .any(|d| d.starts_with("fields: ours") && d.ends_with("GDAL []"))
            {
                eprintln!(
                    "~ {label} / {}: skipped (GDAL too old to read its fields)",
                    entry.name
                );
                continue;
            }
            if !diffs.is_empty() {
                failures += 1;
                eprintln!("✗ {label} / {}: {} differences", entry.name, diffs.len());
                diffs.iter().take(4).for_each(|d| eprintln!("    {d}"));
            }
        }
    }
    eprintln!(
        "{} geodatabases, {tables} tables, {failures} with differences",
        gdbs.len()
    );
    assert_eq!(failures, 0);
}

#[test]
fn sparse_64bit_objectids_are_exact() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/large/filegdb/objectid64/with_holes_8.gdb");
    let Ok(gdb) = Geodatabase::open(&path) else {
        eprintln!("skipping: run scripts/fetch-testdata.sh");
        return;
    };
    let oids = |name: &str| -> Vec<i64> {
        gdb.open_table(name)
            .unwrap()
            .features()
            .map(|f| f.unwrap().oid)
            .collect()
    };
    assert_eq!(oids("with_holes_8_a"), [123456]);
    assert_eq!(oids("with_holes_8_c"), [12345678]);
    assert_eq!(oids("with_holes_8_e"), [1234567890]);
    assert_eq!(
        oids("with_holes_8_f"),
        [123456, 1234567, 12345678, 123456789, 1234567890]
    );
}

fn value_text(v: &Value) -> String {
    match v {
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::Single(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::String(s) => s.clone(),
        other => format!("{other:?}"),
    }
}

fn json_text(j: &Json) -> String {
    match j {
        Json::String(s) => s.clone(),
        Json::Number(n) => n.as_f64().map_or(n.to_string(), |f| f.to_string()),
        other => other.to_string(),
    }
}

/// GDAL's (fieldType, fieldSubType) for an ArcGIS field type.
fn gdal_type(t: FieldType) -> (&'static str, Option<&'static str>) {
    match t {
        FieldType::SmallInteger => ("Integer", Some("Int16")),
        FieldType::Integer => ("Integer", None),
        FieldType::BigInteger => ("Integer64", None),
        FieldType::Single => ("Real", Some("Float32")),
        FieldType::Double => ("Real", None),
        FieldType::Date => ("DateTime", None),
        _ => ("String", None),
    }
}

#[test]
fn schema_matches_gdal() {
    if !ogrinfo_available() || !gdal_version().is_some_and(|v| v >= (3, 6)) {
        eprintln!("skipping: needs GDAL >= 3.6 for domains and relationships");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/large/filegdb");
    let (mut domains, mut rels, mut field_domains, mut diffs) = (0, 0, 0, Vec::new());
    for path in geodatabases(&root) {
        let label = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let (Ok(gdb), Ok(json)) = (Geodatabase::open(&path), ogr_dataset_json(&path)) else {
            continue;
        };

        let empty = serde_json::Map::new();
        let gdal_domains = json["domains"].as_object().unwrap_or(&empty);
        // GDAL cuts domain names at the first '/'; ArcGIS (and we) keep them whole.
        let gdal_name = |n: &str| n.split('/').next().unwrap_or(n).to_owned();
        let ours: Vec<String> = gdb.domains().iter().map(|d| gdal_name(&d.name)).collect();
        let theirs: Vec<String> = gdal_domains.keys().cloned().collect();
        let only_ours: Vec<_> = ours.iter().filter(|n| !theirs.contains(n)).collect();
        let only_gdal: Vec<_> = theirs.iter().filter(|n| !ours.contains(n)).collect();
        if !only_ours.is_empty() || !only_gdal.is_empty() {
            diffs.push(format!(
                "{label}: domains only ours {only_ours:?}, only GDAL {only_gdal:?}"
            ));
        }
        for d in gdb.domains() {
            domains += 1;
            if d.name.contains('/') {
                continue; // several of ours collapse onto one truncated GDAL name
            }
            let Some(g) = gdal_domains.get(&d.name) else {
                continue;
            };
            let mut check = |what: &str, ok: bool| {
                if !ok {
                    diffs.push(format!("{label}: domain {} {what}: GDAL {g}", d.name));
                }
            };
            check(
                "description",
                g["description"].as_str().unwrap_or("") == d.description,
            );
            let (t, sub) = gdal_type(d.field_type);
            check(
                "field type",
                g["fieldType"] == t && g["fieldSubType"].as_str() == sub,
            );
            let split = match d.split_policy {
                SplitPolicy::DefaultValue => "default value",
                SplitPolicy::Duplicate => "duplicate",
                SplitPolicy::GeometryRatio => "geometry ratio",
            };
            let merge = match d.merge_policy {
                MergePolicy::DefaultValue => "default value",
                MergePolicy::SumValues => "sum",
                MergePolicy::AreaWeighted => "geometry weighted",
            };
            check("split policy", g["splitPolicy"] == split);
            check("merge policy", g["mergePolicy"] == merge);
            match &d.kind {
                DomainKind::Coded(values) => {
                    let ours: Vec<(String, String)> = values
                        .iter()
                        .map(|(c, n)| (value_text(c), n.clone()))
                        .collect();
                    let mut theirs: Vec<(String, String)> = g["codedValues"]
                        .as_object()
                        .map(|m| m.iter().map(|(k, v)| (k.clone(), json_text(v))).collect())
                        .unwrap_or_default();
                    let mut ours_sorted = ours.clone();
                    ours_sorted.sort();
                    theirs.sort();
                    check("coded values", ours_sorted == theirs);
                }
                DomainKind::Range { min, max } => {
                    let num = |j: &Json| {
                        j.as_f64()
                            .map(|f| f.to_string())
                            .unwrap_or_else(|| json_text(j))
                    };
                    let ours_min = value_text(min)
                        .parse::<f64>()
                        .map(|f| f.to_string())
                        .unwrap_or(value_text(min));
                    let ours_max = value_text(max)
                        .parse::<f64>()
                        .map(|f| f.to_string())
                        .unwrap_or(value_text(max));
                    check(
                        "range",
                        num(&g["minValue"]) == ours_min && num(&g["maxValue"]) == ours_max,
                    );
                }
            }
        }

        // Field → domain assignments.
        for layer in json["layers"].as_array().into_iter().flatten() {
            let name = layer["name"].as_str().unwrap_or_default();
            let Ok(table) = gdb.open_table(name) else {
                continue;
            };
            for f in layer["fields"].as_array().into_iter().flatten() {
                let fname = f["name"].as_str().unwrap_or_default();
                let ours = table
                    .fields()
                    .iter()
                    .find(|x| x.name == fname)
                    .and_then(|x| x.domain.as_deref());
                if ours.is_some() {
                    field_domains += 1;
                }
                if ours != f["domainName"].as_str() {
                    diffs.push(format!(
                        "{label}/{name}.{fname}: domain ours {ours:?}, GDAL {}",
                        f["domainName"]
                    ));
                }
            }
        }

        // Relationship classes.
        let gdal_rels = json["relationships"].as_object().unwrap_or(&empty);
        let ours: Vec<&str> = gdb
            .relationship_classes()
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        let theirs: Vec<&str> = gdal_rels.keys().map(String::as_str).collect();
        if ours != theirs {
            diffs.push(format!(
                "{label}: relationships ours {ours:?}, GDAL {theirs:?}"
            ));
        }
        for r in gdb.relationship_classes() {
            rels += 1;
            let Some(g) = gdal_rels.get(&r.name) else {
                continue;
            };
            let strs = |k: &str| -> Vec<String> {
                g[k].as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            };
            let cardinality = match r.cardinality {
                Cardinality::OneToOne => "OneToOne",
                Cardinality::OneToMany => "OneToMany",
                Cardinality::ManyToMany => "ManyToMany",
            };
            // GDAL's convention: many-to-many uses a mapping table; an attributed
            // one-to-one/many lists the origin foreign key and destination primary key
            // together as the right table's fields.
            let mapped = r.cardinality == Cardinality::ManyToMany;
            let expected_right: Vec<String> = if mapped {
                r.destination_primary_key.clone().into_iter().collect()
            } else if r.is_attributed {
                std::iter::once(r.origin_foreign_key.clone())
                    .chain(r.destination_primary_key.clone())
                    .collect()
            } else {
                vec![r.origin_foreign_key.clone()]
            };
            let ok = g["cardinality"] == cardinality
                && g["type"]
                    == if r.is_composite {
                        "Composite"
                    } else {
                        "Association"
                    }
                && g["left_table_name"] == r.origin.as_str()
                && g["right_table_name"] == r.destination.as_str()
                && strs("left_table_fields") == [r.origin_primary_key.clone()]
                && strs("right_table_fields") == expected_right
                && (!mapped
                    || (strs("left_mapping_table_fields") == [r.origin_foreign_key.clone()]
                        && strs("right_mapping_table_fields")
                            == r.destination_foreign_key
                                .clone()
                                .into_iter()
                                .collect::<Vec<_>>()))
                && g["forward_path_label"] == r.forward_label.as_str()
                && g["backward_path_label"] == r.backward_label.as_str();
            if !ok {
                diffs.push(format!(
                    "{label}: relationship {}: ours {r:?}, GDAL {g}",
                    r.name
                ));
            }
        }
    }
    eprintln!(
        "{domains} domains, {field_domains} field domain assignments, {rels} relationship classes"
    );
    diffs.iter().for_each(|d| eprintln!("✗ {d}"));
    assert!(diffs.is_empty());
    assert!(
        domains > 0 && rels > 0,
        "test data has domains and relationships"
    );
}
