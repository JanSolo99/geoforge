//! Disable COGO as observed through public ArcGIS tools and saved file states.
use super::{
    catalog::base,
    subtype_edit::{Splice, apply, bad, child_text, load_target, store},
    txn,
};
use crate::table::Table;
use roxmltree::Document;
use std::path::Path;
use unigis_core::{GeometryKind, Result};

/// COGO values/fields stay in place; only their catalog Required markers are removed.
/// No user table, geometry, index, field alias, default or domain is rewritten.
pub fn disable_cogo(dir: impl AsRef<Path>, item: &str) -> Result<()> {
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        let target = load_target(dir, item)?;
        let table = Table::open(base(dir, target.table_id))?;
        match table.geometry_kind() {
            Some(GeometryKind::Polyline) => {}
            None => {
                return Err(bad(
                    "ERROR 000732: Input Line Features: dataset does not exist or is not supported",
                ));
            }
            _ => {
                return Err(bad(
                    "ERROR 003917: The feature class geometry is not of type polyline",
                ));
            }
        }
        drop(table);
        let doc = Document::parse(&target.def)
            .map_err(|e| bad(format!("invalid feature class definition: {e}")))?;
        let mut edits = Vec::new();
        for entry in doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoEx")
        {
            if child_text(entry, "Name").is_some_and(|name| {
                ["Direction", "Distance", "Radius", "ArcLength", "Radius2"]
                    .iter()
                    .any(|f| f.eq_ignore_ascii_case(&name))
            }) {
                for required in entry
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "Required")
                {
                    edits.push(Splice {
                        at: required.range(),
                        with: String::new(),
                    });
                }
            }
        }
        if edits.is_empty() {
            return Ok(());
        }
        store(dir, &target, apply(&target.def, edits))
    })
}

const FIELDS: [(&str, &str); 5] = [
    ("Direction", "Direction"),
    ("Distance", "Distance"),
    ("Radius", "Radius"),
    ("ArcLength", "Arc Length"),
    ("Radius2", "Radius2"),
];

/// Enable COGO, adding missing doubles and retaining existing dimension values.
/// ArcGIS resets the five aliases and marks the fields required.
pub fn enable_cogo(dir: impl AsRef<Path>, item: &str) -> Result<()> {
    use super::subtype_edit::{append_child, child, xml_escape};
    use crate::{GdbFieldType, Geodatabase};
    use std::os::unix::fs::FileExt;
    use unigis_core::{Error, Field, FieldType, Value};
    let dir = dir.as_ref();
    txn::atomic(dir, || {
        let target = load_target(dir, item)?;
        let gdb = Geodatabase::open(dir)?;
        let table = gdb.open_table(item)?;
        match table.geometry_kind() {
            Some(GeometryKind::Polyline) => {}
            None => {
                return Err(bad(
                    "ERROR 000732: Input Line Features: dataset does not exist or is not supported",
                ));
            }
            _ => {
                return Err(bad(
                    "ERROR 003917: The feature class geometry is not of type polyline",
                ));
            }
        }
        for (name, _) in FIELDS {
            if let Some(f) = table
                .all_fields()
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                && f.field_type != GdbFieldType::Float64
            {
                return Err(bad(format!("ERROR 002045: Wrong type on field {name}")));
            }
        }
        let original_flags: Vec<_> = table
            .all_fields()
            .iter()
            .zip(table.field_flag_locations())
            .map(|(f, (_, flags))| (f.name.clone(), *flags))
            .collect();
        let mut fields = table.all_fields().to_vec();
        let mut added = 0;
        let mut rewrite = false;
        for (name, alias) in FIELDS {
            let canonical = (alias != name).then(|| alias.to_string());
            if let Some(f) = fields
                .iter_mut()
                .find(|f| f.name.eq_ignore_ascii_case(name))
            {
                if f.alias != canonical {
                    f.alias = canonical;
                    rewrite = true;
                }
            } else {
                let mut f = Field::new(name, FieldType::Double);
                f.alias = canonical;
                fields.push(super::to_gdb_field(&f)?);
                added += 1;
                rewrite = true;
            }
        }
        let doc = Document::parse(&target.def).map_err(|e| bad(e.to_string()))?;
        let list = child(doc.root_element(), "GPFieldInfoExs")
            .ok_or_else(|| bad("missing GPFieldInfoExs"))?;
        let mut edits = Vec::new();
        let mut new_entries = String::new();
        for (name, alias) in FIELDS {
            let f = fields
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(name))
                .unwrap();
            let entry = list.children().find(|n| {
                n.is_element()
                    && n.tag_name().name() == "GPFieldInfoEx"
                    && child_text(*n, "Name").is_some_and(|s| s.eq_ignore_ascii_case(name))
            });
            if let Some(entry) = entry {
                for (tag, value) in [("AliasName", alias), ("Required", "true")] {
                    if let Some(old) = child(entry, tag)
                        && old.text() != Some(value)
                    {
                        edits.push(Splice {
                            at: old.range(),
                            with: format!("<{tag}>{}</{tag}>", xml_escape(value)),
                        });
                    }
                }
                let missing: String = [("AliasName", alias), ("Required", "true")]
                    .iter()
                    .filter(|(tag, _)| child(entry, tag).is_none())
                    .map(|(tag, value)| format!("<{tag}>{}</{tag}>", xml_escape(value)))
                    .collect();
                if !missing.is_empty() {
                    edits.push(append_child(&target.def, entry, &missing));
                }
            } else {
                new_entries += &format!(
                    "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{}</Name><AliasName>{}</AliasName><FieldType>esriFieldTypeDouble</FieldType><IsNullable>{}</IsNullable><Required>true</Required></GPFieldInfoEx>",
                    xml_escape(&f.name),
                    xml_escape(alias),
                    f.nullable
                );
            }
        }
        if !new_entries.is_empty() {
            edits.push(append_child(&target.def, list, &new_entries));
        }
        let definition = apply(&target.def, edits);
        drop(doc);
        if rewrite {
            // rewrite_core expects a computed grid, as the generic schema writer does.
            // Retaining an old fixed grid suppresses its spatial-index rebuild.
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
                let mut desired = original_flags
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                    .map_or(flags, |(_, v)| *v);
                if FIELDS.iter().any(|(n, _)| f.name.eq_ignore_ascii_case(n)) {
                    desired |= 2;
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
