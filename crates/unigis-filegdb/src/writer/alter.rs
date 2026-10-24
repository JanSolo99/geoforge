//! Altering one field of an existing table (`arcpy.management.AlterField`).
//!
//! As observed through the bridge:
//! - A rename keeps the field's alias, domain, default and attribute index (an index follows the
//!   field; its name stays). Renaming the object ID, the geometry or `Shape_Length`/`Shape_Area`
//!   is accepted and does nothing. A new name that exists already (any case, another field) is
//!   error 002598; a new name that differs from the field's only in case changes nothing.
//! - The alias is changed or, with `clear_alias`, removed (the alias then reads as the name).
//!   Giving both is error 001656. An alias equal to the name stays in the item definition.
//! - The item definition's `GPFieldInfoExs` entry follows the rename; an entry left with nothing
//!   beyond its name, type and nullability is dropped. Subtype field references follow too.
//! - A type change on a table with rows is error 001658, on an empty table it is allowed: the
//!   stored length becomes 0 for numbers, dates and blobs, 38 for a GUID, and the text length
//!   (the given one, else the field's old length) for text.
//! - A text length can grow; shrinking it on a table with rows is error 001624. On an empty
//!   table any length is accepted.
//! - Making a field required (`NON_NULLABLE`) on a table with rows is error 001625.

use std::path::Path;

use roxmltree::Document;
use unigis_core::{Feature, FieldType, Result};

use super::catalog::base;
use super::rewrite::rewrite_core;
use super::subtype_edit::{
    Splice, Target, append_child, apply, bad, child, child_text, children, field, load_target,
    store, xml_escape,
};
use super::{esri_type_name, to_gdb_field};
use crate::table::{GdbField, GdbFieldType, Table};

/// What `AlterField` changes about a field.
#[derive(Debug, Default, Clone)]
pub struct FieldAlteration<'a> {
    pub new_name: Option<&'a str>,
    pub alias: Option<&'a str>,
    pub clear_alias: bool,
    pub new_type: Option<FieldType>,
    pub length: Option<u32>,
    pub nullable: Option<bool>,
}

const RESERVED: &[&str] = &[
    "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "BETWEEN", "BY", "CASE", "CHECK", "COLUMN",
    "CREATE", "DEFAULT", "DELETE", "DESC", "DISTINCT", "DROP", "ELSE", "EXISTS", "FOR", "FROM",
    "GRANT", "GROUP", "HAVING", "IN", "INDEX", "INSERT", "INTO", "IS", "JOIN", "KEY", "LIKE",
    "NOT", "NULL", "ON", "OR", "ORDER", "PRIMARY", "SELECT", "SET", "TABLE", "THEN", "TO", "UNION",
    "UNIQUE", "UPDATE", "VALUES", "VIEW", "WHEN", "WHERE", "WITH",
];

/// A field name as ArcGIS keeps it in a geodatabase: other characters than letters, digits and
/// `_` become `_`, a reserved word gets a trailing `_`, and a leading digit is an error.
pub fn geodatabase_field_name(name: &str) -> Result<String> {
    if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return Err(bad(
            "ERROR 000310: The Field name must not start with a number",
        ));
    }
    let mut out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&out)) {
        out.push('_');
    }
    Ok(out)
}

/// `arcpy.management.AlterFields`: the alterations in order, each one as [`alter_field`] (one that
/// fails leaves the earlier ones in place). Every field must exist before anything is changed.
/// A row that gives no type means LONG and one that gives no length 255 (the value table's
/// defaults in ArcGIS); an alias of `#` is taken literally.
pub fn alter_fields(
    dir: impl AsRef<Path>,
    table: &str,
    rows: &[(&str, FieldAlteration)],
) -> Result<()> {
    let dir = dir.as_ref();
    let t = load_target(dir, table)?;
    for (name, _) in rows {
        field(&t, name).map_err(|_| {
            bad(format!(
                "ERROR 000622: Failed to execute (AlterFields). Parameters are not valid. | \
                 ERROR 000728: Field {name} does not exist within table"
            ))
        })?;
    }
    for (name, alteration) in rows {
        // A row without a type alters the field to LONG (the value table's default), which only
        // an empty table, or a field that is a long already, accepts.
        let mut alteration = alteration.clone();
        alteration.new_type.get_or_insert(FieldType::Integer);
        // ... and one without a length means 255 (matters for text only).
        alteration.length.get_or_insert(255);
        alter_field(dir, table, name, &alteration)?;
    }
    Ok(())
}

/// Alters the field `field_name` of the table `table` of the geodatabase at `dir`.
pub fn alter_field(
    dir: impl AsRef<Path>,
    table: &str,
    field_name: &str,
    alteration: &FieldAlteration,
) -> Result<()> {
    let dir = dir.as_ref();
    if alteration.alias.is_some() && alteration.clear_alias {
        return Err(bad(
            "ERROR 001656: Cannot specify field alias and clear alias parameters simultaneously.",
        ));
    }
    // ArcGIS makes the other changes before it finds that the new name exists.
    let name_taken = super::txn::atomic(dir, || -> Result<Option<unigis_core::Error>> {
        let mut name_taken = None;
        let t = load_target(dir, table)?;
        let (current, _, _) = field(&t, field_name)?.clone();
        let tab_base = base(dir, t.table_id);
        let old = Table::open(&tab_base)?;
        let populated = old.row_count() > 0;
        let shape = old.shape_field().map(|f| f.name.to_ascii_lowercase());
        let mut fields: Vec<GdbField> = old.all_fields().to_vec();
        // The spatial grid is chosen again from the rows.
        for g in fields.iter_mut().filter_map(|f| f.geometry.as_mut()) {
            g.grid_sizes.clear();
        }
        let at = fields
            .iter()
            .position(|f| f.name.eq_ignore_ascii_case(&current))
            .ok_or_else(|| bad(format!("no field {field_name}")))?;
        let special = matches!(
            fields[at].field_type,
            GdbFieldType::ObjectId | GdbFieldType::Geometry
        ) || shape.as_ref().is_some_and(|s| {
            let n = current.to_ascii_lowercase();
            n == format!("{s}_length") || n == format!("{s}_area")
        });

        let mut name = current.clone();
        if let Some(new) = alteration.new_name.filter(|n| !n.is_empty() && *n != "#") {
            let new = geodatabase_field_name(new)?;
            let new = new.as_str();
            if !special && !new.eq_ignore_ascii_case(&current) {
                if fields.iter().any(|f| f.name.eq_ignore_ascii_case(new)) {
                    name_taken = Some(bad(format!(
                        "ERROR 002598: Name: \"{new}\" already exists."
                    )));
                } else {
                    super::validate_field_name(new)?;
                    name = new.to_owned();
                }
            }
        }

        let g = &mut fields[at];
        if !special {
            if let Some(ty) = alteration.new_type
                && ty != g.field_type.esri()
            {
                if populated {
                    return Err(bad(
                        "ERROR 001658: Cannot alter field types on populated tables.",
                    ));
                }
                let mut probe = unigis_core::Field::new(&current, ty);
                probe.length = alteration.length.unwrap_or(0);
                let fresh = to_gdb_field(&probe)?;
                let length = match fresh.field_type {
                    GdbFieldType::String => alteration.length.unwrap_or(g.length),
                    GdbFieldType::Guid | GdbFieldType::GlobalId => 38,
                    _ => 0,
                };
                g.field_type = fresh.field_type;
                g.length = length;
                g.default = None;
            }
            if g.field_type == GdbFieldType::String
                && let Some(len) = alteration.length.filter(|l| *l > 0)
            {
                if len < g.length && populated {
                    return Err(bad("ERROR 001624: Failed to alter field length."));
                }
                g.length = len;
            }
            if let Some(nullable) = alteration.nullable {
                if !nullable && g.nullable && populated {
                    return Err(bad("ERROR 001625: Failed to alter field is nullable."));
                }
                g.nullable = nullable;
            }
        }
        g.name = name.clone();
        if alteration.clear_alias {
            g.alias = None;
        } else if let Some(alias) = alteration.alias.filter(|a| !a.is_empty()) {
            g.alias = Some(alias.to_owned()).filter(|a| *a != name);
        }
        let new_type = g.field_type;
        let new_nullable = g.nullable;

        let renames = if name == current {
            Vec::new()
        } else {
            vec![(current.clone(), name.clone())]
        };
        let rows = old
            .features()
            .map(|f| {
                let f: Feature = f?;
                Ok((Some(f.oid as u64), f))
            })
            .collect::<Result<Vec<_>>>()?;
        rewrite_core(&tab_base, old, fields, &renames, &rows, true)?;

        let def = alter_definition(&t, &current, &name, alteration, new_type, new_nullable)?;
        if def != t.def {
            store(dir, &t, def)?;
        }
        Ok(name_taken)
    })?;
    name_taken.map_or(Ok(()), Err)
}

/// The item definition with the field's `GPFieldInfoEx` entry (and subtype references) changed.
fn alter_definition(
    t: &Target,
    current: &str,
    name: &str,
    alteration: &FieldAlteration,
    new_type: GdbFieldType,
    nullable: bool,
) -> Result<String> {
    let xml = &t.def;
    let doc =
        Document::parse(xml).map_err(|e| bad(format!("the item definition is not XML: {e}")))?;
    let mut edits: Vec<Splice> = Vec::new();

    // Subtype references to the field.
    if name != current {
        for n in doc.descendants().filter(|n| {
            n.is_element() && matches!(n.tag_name().name(), "SubtypeField" | "FieldName")
        }) {
            if n.text()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case(current))
            {
                edits.push(Splice {
                    at: n.range(),
                    with: format!("<{0}>{1}</{0}>", n.tag_name().name(), xml_escape(name)),
                });
            }
        }
    }

    let list = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoExs");
    let esri = esri_type_name(new_type);
    let alias_given = alteration.alias.filter(|a| !a.is_empty());
    if let Some(list) = list {
        let entry = children(list, "GPFieldInfoEx")
            .find(|e| child_text(*e, "Name").is_some_and(|n| n.eq_ignore_ascii_case(current)));
        match entry {
            Some(e) => {
                if name != current
                    && let Some(n) = child(e, "Name")
                {
                    edits.push(Splice {
                        at: n.range(),
                        with: format!("<Name>{}</Name>", xml_escape(name)),
                    });
                }
                let alias_node = child(e, "AliasName");
                if alteration.clear_alias {
                    if let Some(a) = alias_node {
                        edits.push(Splice {
                            at: a.range(),
                            with: String::new(),
                        });
                    }
                } else if let Some(a) = alias_given {
                    let new = format!("<AliasName>{}</AliasName>", xml_escape(a));
                    match alias_node {
                        Some(old) => edits.push(Splice {
                            at: old.range(),
                            with: new,
                        }),
                        None => {
                            if let Some(n) = child(e, "Name") {
                                edits.push(Splice {
                                    at: n.range().end..n.range().end,
                                    with: new,
                                });
                            }
                        }
                    }
                }
                if let Some(ft) = child(e, "FieldType")
                    && child_text(e, "FieldType").as_deref() != Some(esri)
                {
                    edits.push(Splice {
                        at: ft.range(),
                        with: format!("<FieldType>{esri}</FieldType>"),
                    });
                }
                if let Some(nn) = child(e, "IsNullable")
                    && child_text(e, "IsNullable").as_deref() != Some(&nullable.to_string())
                {
                    edits.push(Splice {
                        at: nn.range(),
                        with: format!("<IsNullable>{nullable}</IsNullable>"),
                    });
                }
                // An entry that says nothing beyond name, type and nullability goes (not the
                // object ID's or the shape's).
                let rest_plain = e.children().filter(|c| c.is_element()).all(|c| {
                    let n = c.tag_name().name();
                    matches!(n, "Name" | "FieldType" | "IsNullable")
                        || (n == "AliasName" && alteration.clear_alias)
                });
                let ident = matches!(new_type, GdbFieldType::ObjectId | GdbFieldType::Geometry);
                if rest_plain && !ident && alteration.clear_alias {
                    edits = vec![Splice {
                        at: e.range(),
                        with: String::new(),
                    }];
                }
            }
            None => {
                // A field without an entry gets one for an alias or a changed nullability.
                if alias_given.is_some() || !nullable {
                    let alias = alias_given
                        .map(|a| format!("<AliasName>{}</AliasName>", xml_escape(a)))
                        .unwrap_or_default();
                    let entry = format!(
                        "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{}</Name>\
                         {alias}<FieldType>{esri}</FieldType>\
                         <IsNullable>{nullable}</IsNullable></GPFieldInfoEx>",
                        xml_escape(name)
                    );
                    // Entries follow the table's field order.
                    let order: Vec<&str> = t
                        .fields
                        .iter()
                        .map(|f| {
                            if f.0.eq_ignore_ascii_case(current) {
                                name
                            } else {
                                f.0.as_str()
                            }
                        })
                        .collect();
                    let pos = |n: &str| {
                        order
                            .iter()
                            .position(|f| f.eq_ignore_ascii_case(n))
                            .unwrap_or(usize::MAX)
                    };
                    let mine = pos(name);
                    let before = children(list, "GPFieldInfoEx")
                        .find(|x| child_text(*x, "Name").is_some_and(|n| pos(&n) > mine));
                    edits.push(match before {
                        Some(b) => Splice {
                            at: b.range().start..b.range().start,
                            with: entry,
                        },
                        None => append_child(xml, list, &entry),
                    });
                }
            }
        }
    }
    Ok(apply(xml, edits))
}
