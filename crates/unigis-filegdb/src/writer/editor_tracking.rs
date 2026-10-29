//! Disable editor tracking, based on public ArcGIS tools and saved catalog states.
use super::{
    catalog::{Kind, base, find, load_items, missing},
    subtype_edit::{Splice, apply, bad, child, child_text, load_target, store},
    txn,
};
use crate::table::Table;
use roxmltree::Document;
use std::os::unix::fs::FileExt;
use std::path::Path;
use unigis_core::{Error, Result};

const ROLES: [&str; 4] = [
    "CreatorFieldName",
    "CreatedAtFieldName",
    "EditorFieldName",
    "EditedAtFieldName",
];

/// Disable selected roles (creator, creation date, editor, edit date), keeping all
/// fields/values. A feature dataset updates its classes in one journal transaction.
pub fn disable_editor_tracking(
    dir: impl AsRef<Path>,
    item: &str,
    disable: [bool; 4],
) -> Result<()> {
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        let items = load_items(dir)?;
        let entry = find(&items, item, |k| {
            matches!(k, Kind::FeatureClass | Kind::Table | Kind::Dataset)
        })
        .ok_or_else(|| missing(item))?;
        let names = if entry.kind == Kind::Dataset {
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
                    "editor tracking edits of advanced feature datasets are unsupported",
                ));
            }
            members.iter().map(|i| i.name.clone()).collect::<Vec<_>>()
        } else {
            vec![entry.name.clone()]
        };
        // Preflight every target/XML before writing any catalog row.
        let changes = names
            .iter()
            .map(|name| {
                let target = load_target(dir, name)?;
                let (definition, released) = disabled_definition(&target.def, disable)?;
                let table = Table::open(base(dir, target.table_id))?;
                let patches = table
                    .all_fields()
                    .iter()
                    .zip(table.field_flag_locations())
                    .filter(|(field, (_, flags))| {
                        flags & 4 == 0
                            && released.iter().any(|n| field.name.eq_ignore_ascii_case(n))
                    })
                    .map(|(_, &(at, flags))| (at, flags | 4))
                    .collect::<Vec<_>>();
                Ok((target, definition, patches))
            })
            .collect::<Result<Vec<_>>>()?;
        for (target, definition, patches) in changes {
            if !patches.is_empty() {
                let path = base(dir, target.table_id).with_extension("gdbtable");
                // Existing append journal also backs up the complete field section.
                let file = txn::open_for_append(&path)?;
                for (at, flags) in patches {
                    file.write_all_at(&[flags], at)
                        .map_err(|e| Error::io(&path, e))?;
                }
            }
            if definition != target.def {
                store(dir, &target, definition)?;
            }
        }
        Ok(())
    })
}

fn disabled_definition(xml: &str, disable: [bool; 4]) -> Result<(String, Vec<String>)> {
    let doc = Document::parse(xml).map_err(|e| bad(format!("invalid item definition: {e}")))?;
    let root = doc.root_element();
    let names = ROLES.map(|tag| child_text(root, tag).unwrap_or_default());
    let removed: Vec<_> = names
        .iter()
        .enumerate()
        .filter(|(i, name)| disable[*i] && !name.is_empty())
        .map(|(_, name)| name)
        .collect();
    if removed.is_empty() {
        return Ok((xml.into(), vec![]));
    }
    let mut edits = Vec::new();
    for (i, tag) in ROLES.iter().enumerate() {
        if disable[i] && !names[i].is_empty() {
            let node = child(root, tag).unwrap();
            edits.push(Splice {
                at: node.range(),
                with: format!("<{tag}></{tag}>"),
            });
        }
    }
    let enabled = names
        .iter()
        .enumerate()
        .any(|(i, name)| !disable[i] && !name.is_empty());
    if let Some(node) = child(root, "EditorTrackingEnabled") {
        edits.push(Splice {
            at: node.range(),
            with: format!("<EditorTrackingEnabled>{enabled}</EditorTrackingEnabled>"),
        });
    }
    if let Some(list) = child(root, "GPFieldInfoExs") {
        for entry in list.children().filter(|n| n.is_element()) {
            let name = child_text(entry, "Name").unwrap_or_default();
            if !removed.iter().any(|n| n.eq_ignore_ascii_case(&name))
                || names
                    .iter()
                    .enumerate()
                    .any(|(i, n)| !disable[i] && n.eq_ignore_ascii_case(&name))
            {
                continue;
            }
            let Some(editable) = child(entry, "Editable") else {
                continue;
            };
            // Drop only the property imposed by tracking; aliases, domains,
            // defaults, nullability and required/system markers remain intact.
            let plain = entry.children().filter(|n| n.is_element()).all(|n| {
                matches!(
                    n.tag_name().name(),
                    "Name" | "FieldType" | "IsNullable" | "Editable"
                )
            });
            edits.push(Splice {
                at: if plain {
                    entry.range()
                } else {
                    editable.range()
                },
                with: String::new(),
            });
        }
    }
    let released = removed
        .into_iter()
        .filter(|name| {
            !names
                .iter()
                .enumerate()
                .any(|(i, n)| !disable[i] && n.eq_ignore_ascii_case(name))
        })
        .cloned()
        .collect();
    Ok((apply(xml, edits), released))
}
