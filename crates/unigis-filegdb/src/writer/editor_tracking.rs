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

/// Enable roles on a simple feature class/table. Existing roles and time basis
/// are retained; existing values are never backfilled or replaced.
pub fn enable_editor_tracking(
    dir: impl AsRef<Path>,
    item: &str,
    requested: [&str; 4],
    add_fields: bool,
    utc: bool,
) -> Result<()> {
    use super::subtype_edit::{append_child, xml_escape};
    use crate::{GdbFieldType, Geodatabase};
    use unigis_core::{Field, FieldType, Value};
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        if requested.iter().all(|s| s.is_empty()) {
            return Err(bad(
                "ERROR 001218: No editor tracking property selected for operation",
            ));
        }
        let target = load_target(dir, item)?;
        if load_items(dir)?.rows.iter().any(|r| {
            r.name
                .eq_ignore_ascii_case(&format!("{}__ATTACH", target.item.name))
        }) {
            return Err(bad(
                "editor tracking of classes with attachments is unsupported",
            ));
        }
        let gdb = Geodatabase::open(dir)?;
        let table = gdb.open_table(item)?;
        let doc = Document::parse(&target.def).map_err(|e| bad(e.to_string()))?;
        let root = doc.root_element();
        let old = ROLES.map(|tag| child_text(root, tag).unwrap_or_default());
        let active = child_text(root, "EditorTrackingEnabled").as_deref() == Some("true");
        let roles: [String; 4] = std::array::from_fn(|i| {
            if active && !old[i].is_empty() {
                old[i].clone()
            } else {
                requested[i].into()
            }
        });
        for names in [requested.map(str::to_string), roles.clone()] {
            let mut used = std::collections::HashSet::new();
            for name in names.iter().filter(|n| !n.is_empty()) {
                if !used.insert(name.to_lowercase()) {
                    return Err(bad(
                        "ERROR 001214: Invalid input field names. Editor tracking field names should be unique",
                    ));
                }
            }
        }
        let old_flags: Vec<_> = table
            .all_fields()
            .iter()
            .zip(table.field_flag_locations())
            .map(|(f, (_, v))| (f.name.clone(), *v))
            .collect();
        let mut fields = table.all_fields().to_vec();
        let mut added = 0;
        for (i, name) in roles.iter().enumerate().filter(|(_, n)| !n.is_empty()) {
            if let Some(field) = fields.iter().find(|f| f.name.eq_ignore_ascii_case(name)) {
                let correct = if i % 2 == 0 {
                    field.field_type == GdbFieldType::String
                } else {
                    field.field_type == GdbFieldType::DateTime && !field.high_precision
                };
                if !correct {
                    return Err(bad(
                        "ERROR 001217: Invalid Editor Tracking field in dataset",
                    ));
                }
            } else {
                if !add_fields {
                    return Err(bad(
                        "ERROR 001278: Field does not exist. Check add field check box to add new field",
                    ));
                }
                super::validate_field_name(name)?;
                let mut field = Field::new(
                    name,
                    if i % 2 == 0 {
                        FieldType::String
                    } else {
                        FieldType::Date
                    },
                );
                if i % 2 == 0 {
                    field.length = 255;
                }
                fields.push(super::to_gdb_field(&field)?);
                added += 1;
            }
        }
        let roles = roles.map(|n| {
            fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(&n))
                .map_or(n.clone(), |f| f.name.clone())
        });
        let mut edits = Vec::new();
        let mut new_root = String::new();
        let time = if active {
            child_text(root, "IsTimeInUTC").as_deref() == Some("true")
        } else {
            utc
        };
        for (tag, value) in ROLES.into_iter().zip(roles.iter().cloned()).chain([
            ("EditorTrackingEnabled", "true".into()),
            ("IsTimeInUTC", time.to_string()),
        ]) {
            let xml = format!("<{tag}>{}</{tag}>", xml_escape(&value));
            if let Some(node) = child(root, tag) {
                if node.text().unwrap_or("") != value {
                    edits.push(Splice {
                        at: node.range(),
                        with: xml,
                    });
                }
            } else {
                new_root += &xml;
            }
        }
        if !new_root.is_empty() {
            // Native definitions use a prefixed root; locate its actual closing
            // tag rather than assuming an unprefixed </DETableInfo> spelling.
            let at = target
                .def
                .rfind("</")
                .ok_or_else(|| bad("missing root closing tag"))?;
            edits.push(Splice {
                at: at..at,
                with: new_root,
            });
        }
        let list = child(root, "GPFieldInfoExs").ok_or_else(|| bad("missing GPFieldInfoExs"))?;
        let mut entries = String::new();
        // Follow physical field order, including pre-existing fields in arbitrary order.
        for field in fields
            .iter()
            .filter(|f| roles.iter().any(|n| f.name.eq_ignore_ascii_case(n)))
        {
            if let Some(entry) = list.children().find(|n| {
                n.is_element()
                    && n.tag_name().name() == "GPFieldInfoEx"
                    && child_text(*n, "Name").is_some_and(|s| s.eq_ignore_ascii_case(&field.name))
            }) {
                if let Some(node) = child(entry, "Editable") {
                    if node.text() != Some("false") {
                        edits.push(Splice {
                            at: node.range(),
                            with: "<Editable>false</Editable>".into(),
                        });
                    }
                } else {
                    edits.push(append_child(
                        &target.def,
                        entry,
                        "<Editable>false</Editable>",
                    ));
                }
            } else {
                entries += &format!(
                    "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{}</Name><FieldType>{}</FieldType><IsNullable>{}</IsNullable><Editable>false</Editable></GPFieldInfoEx>",
                    xml_escape(&field.name),
                    super::esri_type_name(field.field_type),
                    field.nullable
                );
            }
        }
        if !entries.is_empty() {
            edits.push(append_child(&target.def, list, &entries));
        }
        let definition = apply(&target.def, edits);
        drop(doc);
        if added > 0 {
            for geometry in fields.iter_mut().filter_map(|f| f.geometry.as_mut()) {
                geometry.grid_sizes.clear();
            }
            let rows = table
                .features()
                .map(|r| {
                    let mut r = r?;
                    r.attributes.extend(std::iter::repeat_n(Value::Null, added));
                    Ok((Some(r.oid as u64), r))
                })
                .collect::<Result<Vec<_>>>()?;
            super::rewrite::rewrite_core(
                &base(dir, target.table_id),
                table,
                fields,
                &[],
                &|_| {},
                &rows,
                true,
            )?;
        } else {
            drop(table);
        }
        drop(gdb);
        let path = base(dir, target.table_id).with_extension("gdbtable");
        let updated = Table::open(&path)?;
        let patches: Vec<_> = updated
            .all_fields()
            .iter()
            .zip(updated.field_flag_locations())
            .filter_map(|(f, &(at, flags))| {
                let mut desired = old_flags
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                    .map_or(flags, |(_, v)| *v);
                if roles.iter().any(|n| f.name.eq_ignore_ascii_case(n)) {
                    desired &= !4;
                }
                (desired != flags).then_some((at, desired))
            })
            .collect();
        drop(updated);
        if !patches.is_empty() {
            let file = txn::open_for_append(&path)?;
            for (at, flags) in patches {
                file.write_all_at(&[flags], at)
                    .map_err(|e| Error::io(&path, e))?;
            }
        }
        if definition != target.def {
            store(dir, &target, definition)?;
        }
        Ok(())
    })
}
