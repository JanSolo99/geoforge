//! Public ArcGIS date precision migration: field metadata changes, no row rewrite.
use super::{
    subtype_edit::{
        Splice, append_child, apply, bad, child, child_text, load_target, store, xml_escape,
    },
    txn,
};
use crate::GdbFieldType;
use roxmltree::Document;
use std::{collections::HashSet, path::Path};
use unigis_core::Result;

/// Mark existing Date fields high precision, preserving physical data/index bytes.
/// Preflights the whole list; mixed invalid requests do not partly migrate fields.
pub fn migrate_date_fields_to_high_precision(
    dir: impl AsRef<Path>,
    item: &str,
    fields: &[String],
) -> Result<()> {
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        if fields.is_empty() {
            return Err(bad("ERROR 000735: Date Fields: Value is required"));
        }
        let mut selected = HashSet::new();
        for name in fields {
            if !selected.insert(name.to_lowercase()) {
                return Err(bad("ERROR 000400: Duplicate inputs are not allowed"));
            }
        }
        let target = load_target(dir, item)?;
        for name in fields {
            let field = target
                .fields
                .iter()
                .find(|f| f.0.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    bad(format!(
                        "ERROR 000728: Field {name} does not exist within table"
                    ))
                })?;
            if field.1 != GdbFieldType::DateTime {
                return Err(bad(format!(
                    "ERROR 000640: Invalid field type for {name}; must be of type DATE"
                )));
            }
        }
        let doc = Document::parse(&target.def).map_err(|e| bad(e.to_string()))?;
        let list = child(doc.root_element(), "GPFieldInfoExs")
            .ok_or_else(|| bad("missing GPFieldInfoExs"))?;
        let mut edits = Vec::new();
        let mut new_entries = String::new();
        for (name, _, nullable) in target
            .fields
            .iter()
            .filter(|f| selected.contains(&f.0.to_lowercase()))
        {
            let entry = list.children().find(|n| {
                n.is_element()
                    && n.tag_name().name() == "GPFieldInfoEx"
                    && child_text(*n, "Name").is_some_and(|s| s.eq_ignore_ascii_case(name))
            });
            if let Some(entry) = entry {
                if let Some(old) = child(entry, "HighPrecision") {
                    if old.text() != Some("true") {
                        edits.push(Splice {
                            at: old.range(),
                            with: "<HighPrecision>true</HighPrecision>".into(),
                        });
                    }
                } else {
                    edits.push(append_child(
                        &target.def,
                        entry,
                        "<HighPrecision>true</HighPrecision>",
                    ));
                }
            } else {
                new_entries += &format!(
                    "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{}</Name><FieldType>esriFieldTypeDate</FieldType><IsNullable>{nullable}</IsNullable><HighPrecision>true</HighPrecision></GPFieldInfoEx>",
                    xml_escape(name)
                );
            }
        }
        if !new_entries.is_empty() {
            edits.push(append_child(&target.def, list, &new_entries));
        }
        if edits.is_empty() {
            return Ok(());
        }
        store(dir, &target, apply(&target.def, edits))
    })
}
