//! Add Global IDs, from public ArcGIS controls; one journal covers the input list.
use super::{
    catalog::{Kind, base, find, load_items},
    subtype_edit::{Splice, append_child, apply, bad, child, load_target, store},
    txn,
};
use crate::{GdbFieldType, Geodatabase, Table};
use roxmltree::Document;
use std::{collections::HashSet, fs::File, io::Read, os::unix::fs::FileExt, path::Path};
use unigis_core::{Error, Field, FieldType, Result, Value};

/// Adds a required, noneditable GlobalID and its index to simple classes/tables.
/// Existing GlobalIDs are retained. Feature datasets expand to their simple members.
/// All names belong to this geodatabase; a failure rolls back the complete batch.
pub fn add_global_ids(dir: impl AsRef<Path>, names: &[String]) -> Result<()> {
    let dir = dir.as_ref();
    if names.is_empty() {
        return Err(bad(
            "ERROR 000242: The input dataset, file or layer is invalid",
        ));
    }
    txn::atomic(dir, || {
        let items = load_items(dir)?;
        let mut targets = Vec::new();
        let mut seen = HashSet::new();
        for name in names {
            let entry = find(&items, name, |k| {
                matches!(k, Kind::FeatureClass | Kind::Table | Kind::Dataset)
            })
            .ok_or_else(|| bad("ERROR 000242: The input dataset, file or layer is invalid"))?;
            let members = if entry.kind == Kind::Dataset {
                let prefix = format!("{}\\", entry.path.to_lowercase());
                let members: Vec<_> = items
                    .rows
                    .iter()
                    .filter(|i| i.path.to_lowercase().starts_with(&prefix))
                    .collect();
                if members
                    .iter()
                    .any(|i| !matches!(i.kind, Kind::FeatureClass | Kind::Table))
                {
                    return Err(bad(
                        "GlobalID edits of advanced feature datasets are unsupported",
                    ));
                }
                members.iter().map(|i| i.name.clone()).collect()
            } else {
                vec![entry.name.clone()]
            };
            for member in members {
                if seen.insert(member.to_lowercase()) {
                    targets.push(member);
                }
            }
        }
        for name in targets {
            add_one(dir, &name)?;
        }
        Ok(())
    })
}

fn add_one(dir: &Path, name: &str) -> Result<()> {
    let target = load_target(dir, name)?;
    let gdb = Geodatabase::open(dir)?;
    let table = gdb.open_table(name)?;
    if table
        .all_fields()
        .iter()
        .any(|f| f.field_type == GdbFieldType::GlobalId)
    {
        return Ok(());
    }
    let mut name = "GlobalID".to_owned();
    let mut suffix = 0;
    while table
        .all_fields()
        .iter()
        .any(|f| f.name.eq_ignore_ascii_case(&name))
    {
        suffix += 1;
        name = format!("GlobalID_{suffix}");
    }
    let doc = Document::parse(&target.def).map_err(|e| bad(e.to_string()))?;
    let root = doc.root_element();
    let list = child(root, "GPFieldInfoExs").ok_or_else(|| bad("missing GPFieldInfoExs"))?;
    let entry = format!(
        "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{name}</Name><FieldType>esriFieldTypeGlobalID</FieldType><IsNullable>false</IsNullable><Required>true</Required></GPFieldInfoEx>"
    );
    let mut edits = vec![append_child(&target.def, list, &entry)];
    let mut missing = String::new();
    for (tag, value) in [
        ("HasGlobalID", "true"),
        ("GlobalIDFieldName", name.as_str()),
    ] {
        let with = format!("<{tag}>{value}</{tag}>");
        if let Some(old) = child(root, tag) {
            edits.push(Splice {
                at: old.range(),
                with,
            });
        } else {
            missing += &with;
        }
    }
    if !missing.is_empty() {
        edits.push(append_child(&target.def, root, &missing));
    }
    let definition = apply(&target.def, edits);
    let original_flags: Vec<_> = table
        .all_fields()
        .iter()
        .zip(table.field_flag_locations())
        .map(|(f, (_, flags))| (f.name.clone(), *flags))
        .collect();
    let mut fields = table.fields().to_vec();
    let mut field = Field::new(&name, FieldType::GlobalId);
    field.length = 38;
    field.nullable = false;
    fields.push(field);
    // OS entropy, rather than the deterministic geoprocessing sampling RNG.
    let mut entropy = File::open("/dev/urandom").map_err(|e| Error::io("/dev/urandom", e))?;
    let rows = table
        .features()
        .map(|r| {
            let mut r = r?;
            let mut bytes = [0; 16];
            entropy
                .read_exact(&mut bytes)
                .map_err(|e| Error::io("/dev/urandom", e))?;
            bytes[6] = (bytes[6] & 15) | 64;
            bytes[8] = (bytes[8] & 63) | 128;
            let h = bytes.map(|b| format!("{b:02X}"));
            let id = format!(
                "{{{}-{}-{}-{}-{}}}",
                h[..4].concat(),
                h[4..6].concat(),
                h[6..8].concat(),
                h[8..10].concat(),
                h[10..].concat()
            );
            r.attributes.push(Value::Guid(id));
            Ok((Some(r.oid as u64), r))
        })
        .collect::<Result<Vec<_>>>()?;
    drop(table);
    drop(gdb);
    super::rewrite_table(base(dir, target.table_id), &fields, &rows)?;
    super::add_attribute_index(dir, &target.item.name, &name, &format!("FDO_{name}"))?;
    let path = base(dir, target.table_id).with_extension("gdbtable");
    let updated = Table::open(&path)?;
    let patches: Vec<_> = updated
        .all_fields()
        .iter()
        .zip(updated.field_flag_locations())
        .filter_map(|(f, &(at, flags))| {
            let desired = if f.name == name {
                flags & !4
            } else {
                original_flags
                    .iter()
                    .find(|(old, _)| old.eq_ignore_ascii_case(&f.name))
                    .map_or(flags, |(_, v)| *v)
            };
            (desired != flags).then_some((at, desired))
        })
        .collect();
    drop(updated);
    let file = txn::open_for_append(&path)?;
    for (at, flags) in patches {
        file.write_all_at(&[flags], at)
            .map_err(|e| Error::io(&path, e))?;
    }
    store(dir, &target, definition)
}
