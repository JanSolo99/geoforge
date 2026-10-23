//! Editing subtypes, field domains and field defaults of an existing table
//! (`arcpy.management.SetSubtypeField`, `AddSubtype`, `RemoveSubtype`, `SetDefaultSubtype`,
//! `AssignDomainToField`, `RemoveDomainFromField`, `AssignDefaultToField`).
//!
//! All of it lives in the table's item definition (`GDB_Items.Definition`), as observed through
//! the bridge:
//! - `GPFieldInfoExs` lists, in table order, only the fields with something to say (a domain, a
//!   default, an alias, ...); an entry that loses its last property is dropped.
//! - The subtype field's `DefaultValueNumeric` follows the default subtype code.
//! - A new subtype starts with a `SubtypeFieldInfo` for every field that has a default.
//! - Removing a subtype's domain removes its whole `SubtypeFieldInfo` (default included).
//! - Removing the last subtype leaves the subtype field set, with default code 0.
//! - A field-level default is also written to the table's own field definition; the default
//!   subtype is not.
//! - Domains a table uses are linked to it in `GDB_ItemRelationships`.

use std::collections::HashSet;
use std::ops::Range;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use roxmltree::{Document, Node};
use unigis_core::{Error, Feature, FieldType, Result, Value};

use super::catalog::{ItemRow, Kind, base, column, find, load_items, missing, text};
use super::edit::TableEditor;
use super::rewrite::rewrite_table;
use super::{REL_DOMAIN_IN_DATASET, esri_type_name, relationship_values};
use crate::schema::{Domain, DomainKind};
use crate::table::{GdbFieldType, Table, decode_default};

fn bad(msg: impl Into<String>) -> Error {
    Error::Unsupported(msg.into())
}

// ---------------------------------------------------------------------------------------------
// XML splicing

struct Splice {
    at: Range<usize>,
    with: String,
}

fn apply(xml: &str, mut edits: Vec<Splice>) -> String {
    edits.sort_by_key(|e| std::cmp::Reverse(e.at.start));
    let mut out = xml.to_string();
    for e in edits {
        out.replace_range(e.at, &e.with);
    }
    out
}

fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn child_text(n: Node, name: &str) -> Option<String> {
    child(n, name).map(|c| c.text().unwrap_or("").trim().to_string())
}

fn children<'a>(n: Node<'a, 'a>, name: &'a str) -> impl Iterator<Item = Node<'a, 'a>> {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

/// `xml` with `new` added as the last content of the element `n`.
fn append_child(xml: &str, n: Node, new: &str) -> Splice {
    let r = n.range();
    let slice = &xml[r.clone()];
    let tag = n.tag_name().name();
    if let Some(open) = slice.strip_suffix("/>") {
        Splice {
            at: r,
            with: format!("{}>{new}</{tag}>", open.trim_end()),
        }
    } else {
        let close = format!("</{tag}>");
        let at = r.end - close.len();
        Splice {
            at: at..at,
            with: new.to_string(),
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Where a definition's root element ends (before its closing tag).
fn before_root_close(xml: &str) -> usize {
    xml.rfind("</").unwrap_or(xml.len())
}

// ---------------------------------------------------------------------------------------------
// the table being edited

struct Target {
    item: ItemRow,
    table_id: u64,
    /// (name, type) of every field, in table order.
    fields: Vec<(String, GdbFieldType, bool)>,
    def: String,
}

fn xs_type(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "xs:short",
        FieldType::Integer => "xs:int",
        FieldType::BigInteger => "xs:long",
        FieldType::Single => "xs:float",
        FieldType::Double => "xs:double",
        _ => "xs:string",
    }
}

fn value_text(v: &Value) -> Option<String> {
    Some(match v {
        Value::SmallInteger(n) => n.to_string(),
        Value::Integer(n) => n.to_string(),
        Value::BigInteger(n) => n.to_string(),
        Value::Single(n) => n.to_string(),
        Value::Double(n) => n.to_string(),
        Value::String(s) => xml_escape(s),
        _ => return None,
    })
}

/// `v` as a value of the field type `t`.
fn coerce(v: &Value, t: GdbFieldType) -> Result<Value> {
    use GdbFieldType as G;
    let int = match v {
        Value::SmallInteger(n) => Some(i64::from(*n)),
        Value::Integer(n) => Some(i64::from(*n)),
        Value::BigInteger(n) => Some(*n),
        _ => None,
    };
    let float = match v {
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        other => int.map(|i| i as f64).filter(|_| {
            matches!(
                other,
                Value::SmallInteger(_) | Value::Integer(_) | Value::BigInteger(_)
            )
        }),
    };
    let wrong = || bad("ERROR 000800: the default value does not fit the field's type");
    Ok(match t {
        G::Int16 => {
            Value::SmallInteger(i16::try_from(int.ok_or_else(wrong)?).map_err(|_| wrong())?)
        }
        G::Int32 => Value::Integer(i32::try_from(int.ok_or_else(wrong)?).map_err(|_| wrong())?),
        G::Int64 => Value::BigInteger(int.ok_or_else(wrong)?),
        G::Float32 => Value::Single(float.ok_or_else(wrong)? as f32),
        G::Float64 => Value::Double(float.ok_or_else(wrong)?),
        G::String => match v {
            Value::String(s) => Value::String(s.clone()),
            _ => return Err(wrong()),
        },
        _ => return Err(bad("this field type has no default value")),
    })
}

fn default_element(v: &Value) -> String {
    match v {
        Value::String(s) => format!("<DefaultValueString>{}</DefaultValueString>", xml_escape(s)),
        other => format!(
            "<DefaultValueNumeric>{}</DefaultValueNumeric>",
            value_text(other).unwrap_or_default()
        ),
    }
}

fn load_target(dir: &Path, table: &str) -> Result<Target> {
    let items = load_items(dir)?;
    let item = find(&items, table, |k| {
        matches!(k, Kind::FeatureClass | Kind::Table)
    })
    .ok_or_else(|| missing(table))?;
    let cat = Table::open(base(dir, 1))?;
    let table_id = cat
        .features()
        .filter_map(|r| r.ok())
        .find(|r| text(&r.attributes[0]).eq_ignore_ascii_case(&item.name))
        .map(|r| r.oid as u64)
        .ok_or_else(|| missing(table))?;
    let t = Table::open(base(dir, table_id))?;
    let fields = t
        .all_fields()
        .iter()
        .map(|f| (f.name.clone(), f.field_type, f.nullable))
        .collect();
    let def = text(&item.row.attributes[items.c_def]);
    let c_def = items.c_def;
    let _ = c_def;
    let item = ItemRow {
        row: item.row.clone(),
        uuid: item.uuid.clone(),
        kind: item.kind,
        name: item.name.clone(),
        path: item.path.clone(),
    };
    Ok(Target {
        item,
        table_id,
        fields,
        def,
    })
}

fn field<'a>(t: &'a Target, name: &str) -> Result<&'a (String, GdbFieldType, bool)> {
    t.fields
        .iter()
        .find(|f| f.0.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            bad(format!(
                "ERROR 000728: Field {name} does not exist within table"
            ))
        })
}

// ---------------------------------------------------------------------------------------------
// GPFieldInfoEx entries

enum Change<'a> {
    Keep,
    Set(&'a str),
    Remove,
}

enum DefaultChange<'a> {
    Keep,
    Set(&'a Value),
    Remove,
}

/// The definition with the field-level domain and default of `field` changed.
fn change_field_info(
    t: &Target,
    xml: &str,
    name: &str,
    domain: Change,
    default: DefaultChange,
) -> Result<String> {
    let (fname, ftype, nullable) = field(t, name)?.clone();
    let doc =
        Document::parse(xml).map_err(|e| bad(format!("the item definition is not XML: {e}")))?;
    let list = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoExs")
        .ok_or_else(|| bad("the item definition has no GPFieldInfoExs"))?;
    let entry = children(list, "GPFieldInfoEx")
        .find(|e| child_text(*e, "Name").is_some_and(|n| n.eq_ignore_ascii_case(&fname)));
    let esri = esri_type_name(ftype);
    let domain_removed = matches!(domain, Change::Remove);
    match entry {
        None => {
            if matches!(
                (&domain, &default),
                (
                    Change::Keep | Change::Remove,
                    DefaultChange::Keep | DefaultChange::Remove
                )
            ) {
                return Ok(xml.to_string());
            }
            let mut e = format!(
                "<GPFieldInfoEx xsi:type='typens:GPFieldInfoEx'><Name>{}</Name>",
                xml_escape(&fname)
            );
            if let Change::Set(d) = domain {
                e += &format!("<DomainName>{}</DomainName>", xml_escape(d));
            }
            e += &format!("<FieldType>{esri}</FieldType>");
            if let DefaultChange::Set(v) = default {
                e += &default_element(v);
            }
            e += &format!("<IsNullable>{nullable}</IsNullable></GPFieldInfoEx>");
            // Entries follow the table's field order.
            let pos = |n: &str| {
                t.fields
                    .iter()
                    .position(|f| f.0.eq_ignore_ascii_case(n))
                    .unwrap_or(usize::MAX)
            };
            let mine = pos(&fname);
            let before = children(list, "GPFieldInfoEx")
                .find(|x| child_text(*x, "Name").is_some_and(|n| pos(&n) > mine));
            let splice = match before {
                Some(b) => Splice {
                    at: b.range().start..b.range().start,
                    with: e,
                },
                None => append_child(xml, list, &e),
            };
            Ok(apply(xml, vec![splice]))
        }
        Some(entry) => {
            let mut edits = Vec::new();
            match domain {
                Change::Keep => {}
                Change::Remove => {
                    if let Some(d) = child(entry, "DomainName") {
                        edits.push(Splice {
                            at: d.range(),
                            with: String::new(),
                        });
                    }
                }
                Change::Set(d) => {
                    let new = format!("<DomainName>{}</DomainName>", xml_escape(d));
                    match child(entry, "DomainName") {
                        Some(old) => edits.push(Splice {
                            at: old.range(),
                            with: new,
                        }),
                        None => {
                            let after = child(entry, "AliasName")
                                .or_else(|| child(entry, "Name"))
                                .map(|n| n.range().end)
                                .unwrap_or(0);
                            edits.push(Splice {
                                at: after..after,
                                with: new,
                            });
                        }
                    }
                }
            }
            let defaults: Vec<Node> = entry
                .children()
                .filter(|c| {
                    c.is_element()
                        && matches!(
                            c.tag_name().name(),
                            "DefaultValueNumeric" | "DefaultValueString"
                        )
                })
                .collect();
            match default {
                DefaultChange::Keep => {}
                DefaultChange::Remove => edits.extend(defaults.iter().map(|d| Splice {
                    at: d.range(),
                    with: String::new(),
                })),
                DefaultChange::Set(v) => {
                    let new = default_element(v);
                    match defaults.split_first() {
                        Some((first, rest)) => {
                            edits.push(Splice {
                                at: first.range(),
                                with: new,
                            });
                            edits.extend(rest.iter().map(|d| Splice {
                                at: d.range(),
                                with: String::new(),
                            }));
                        }
                        None => {
                            let after = child(entry, "FieldType")
                                .map(|n| n.range().end)
                                .unwrap_or(0);
                            edits.push(Splice {
                                at: after..after,
                                with: new,
                            });
                        }
                    }
                }
            }
            let out = apply(xml, edits);
            drop(doc);
            // An entry emptied by removing its domain goes; one emptied by clearing its default
            // stays (ArcGIS keeps it).
            if domain_removed {
                drop_empty_entries(t, &out)
            } else {
                Ok(out)
            }
        }
    }
}

/// Entries of `GPFieldInfoExs` that say nothing beyond their name and type are dropped (but never
/// the object ID's or the shape's).
fn drop_empty_entries(t: &Target, xml: &str) -> Result<String> {
    let doc = Document::parse(xml).map_err(|e| bad(e.to_string()))?;
    let Some(list) = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoExs")
    else {
        return Ok(xml.to_string());
    };
    let mut edits = Vec::new();
    for e in children(list, "GPFieldInfoEx") {
        let name = child_text(e, "Name").unwrap_or_default();
        let ftype = t
            .fields
            .iter()
            .find(|f| f.0.eq_ignore_ascii_case(&name))
            .map(|f| f.1);
        if matches!(
            ftype,
            Some(GdbFieldType::ObjectId | GdbFieldType::Geometry) | None
        ) {
            continue;
        }
        let plain = e
            .children()
            .filter(|c| c.is_element())
            .all(|c| matches!(c.tag_name().name(), "Name" | "FieldType" | "IsNullable"));
        if plain {
            edits.push(Splice {
                at: e.range(),
                with: String::new(),
            });
        }
    }
    Ok(apply(xml, edits))
}

/// The field-level defaults the definition gives, by field name.
fn xml_defaults(t: &Target) -> Vec<(String, Value)> {
    let Ok(doc) = Document::parse(&t.def) else {
        return Vec::new();
    };
    let Some(list) = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoExs")
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in children(list, "GPFieldInfoEx") {
        let Some(name) = child_text(e, "Name") else {
            continue;
        };
        let Some((_, ft, _)) = t.fields.iter().find(|f| f.0.eq_ignore_ascii_case(&name)) else {
            continue;
        };
        let raw =
            child_text(e, "DefaultValueNumeric").or_else(|| child_text(e, "DefaultValueString"));
        let Some(raw) = raw else { continue };
        let v = match ft {
            GdbFieldType::String => Some(Value::String(raw)),
            GdbFieldType::Int16 => raw.parse().ok().map(Value::SmallInteger),
            GdbFieldType::Int32 => raw.parse().ok().map(Value::Integer),
            GdbFieldType::Int64 => raw.parse().ok().map(Value::BigInteger),
            GdbFieldType::Float32 => raw.parse().ok().map(Value::Single),
            GdbFieldType::Float64 => raw.parse().ok().map(Value::Double),
            _ => None,
        };
        if let Some(v) = v {
            out.push((name, v));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// subtypes

fn subtype_field(xml: &str) -> Option<String> {
    let doc = Document::parse(xml).ok()?;
    let n = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "SubtypeFieldName")?;
    n.text()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn subtype_codes(xml: &str) -> Vec<i64> {
    let Ok(doc) = Document::parse(xml) else {
        return Vec::new();
    };
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Subtype")
        .filter_map(|n| child_text(n, "SubtypeCode")?.parse().ok())
        .collect()
}

fn default_code(xml: &str) -> i64 {
    Document::parse(xml)
        .ok()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "DefaultSubtypeCode")
                .and_then(|n| n.text()?.trim().parse().ok())
        })
        .unwrap_or(0)
}

fn no_subtype_field() -> Error {
    bad("ERROR 000189: Table does not have a subtype field")
}

fn invalid_code() -> Error {
    bad("ERROR 000192: Invalid value for Subtype Code")
}

fn code_value(t: &Target, code: i64) -> Result<Value> {
    let name = subtype_field(&t.def).ok_or_else(no_subtype_field)?;
    coerce(&Value::BigInteger(code), field(t, &name)?.1)
}

/// Replaces the text of a single-valued element, or adds it before the root's closing tag.
fn set_simple(xml: &str, tag: &str, value: &str) -> String {
    let Ok(doc) = Document::parse(xml) else {
        return xml.to_string();
    };
    match doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
    {
        Some(n) => apply(
            xml,
            vec![Splice {
                at: n.range(),
                with: format!("<{tag}>{}</{tag}>", xml_escape(value)),
            }],
        ),
        None => {
            let at = before_root_close(xml);
            apply(
                xml,
                vec![Splice {
                    at: at..at,
                    with: format!("<{tag}>{}</{tag}>", xml_escape(value)),
                }],
            )
        }
    }
}

fn store(dir: &Path, t: &Target, def: String) -> Result<()> {
    let items = load_items(dir)?;
    let mut row: Feature = t.item.row.clone();
    row.attributes[items.c_def] = Value::Xml(def.clone());
    let mut ed = TableEditor::open(base(dir, 4))?;
    ed.update(t.item.row.oid as u64, &row)?;
    ed.finish()?;
    sync_domain_links(dir, t, &def)
}

/// Domains the definition names, each linked to the table (and no others).
fn sync_domain_links(dir: &Path, t: &Target, def: &str) -> Result<()> {
    let items = load_items(dir)?;
    let used: HashSet<String> = Document::parse(def)
        .map(|d| {
            d.descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "DomainName")
                .filter_map(|n| n.text().map(|s| s.trim().to_lowercase()))
                .collect()
        })
        .unwrap_or_default();
    let want: HashSet<String> = items
        .rows
        .iter()
        .filter(|i| i.kind == Kind::Domain && used.contains(&i.name.to_lowercase()))
        .map(|i| i.uuid.clone())
        .collect();
    let rels = Table::open(base(dir, 6))?;
    let (c_origin, c_dest, c_type) = (
        column(&rels, "OriginID")?,
        column(&rels, "DestID")?,
        column(&rels, "Type")?,
    );
    let mut have: HashSet<String> = HashSet::new();
    let mut remove = Vec::new();
    for r in rels.features() {
        let r = r?;
        let ty = text(&r.attributes[c_type]);
        if text(&r.attributes[c_origin]) == t.item.uuid
            && ty.len() >= 10
            && ty[..10].eq_ignore_ascii_case(&REL_DOMAIN_IN_DATASET[..10])
        {
            let dest = text(&r.attributes[c_dest]);
            if want.contains(&dest) {
                have.insert(dest);
            } else {
                remove.push(r.oid as u64);
            }
        }
    }
    let add: Vec<&String> = want.iter().filter(|d| !have.contains(*d)).collect();
    if add.is_empty() && remove.is_empty() {
        return Ok(());
    }
    let mut ed = TableEditor::open(base(dir, 6))?;
    for oid in remove {
        ed.delete(oid)?;
    }
    for d in add {
        let attributes =
            relationship_values(&random_uuid(), &t.item.uuid, d, REL_DOMAIN_IN_DATASET);
        ed.insert(&Feature {
            oid: 0,
            geometry: None,
            attributes,
        })?;
    }
    ed.finish()?;
    Ok(())
}

fn random_uuid() -> String {
    static N: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    let mut x = t
        ^ (u64::from(std::process::id()) << 32)
        ^ N.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed);
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let (a, b) = (next(), next());
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&a.to_le_bytes());
    bytes[8..].copy_from_slice(&b.to_le_bytes());
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let h: Vec<String> = bytes.iter().map(|b| format!("{b:02X}")).collect();
    format!(
        "{{{}-{}-{}-{}-{}}}",
        h[..4].concat(),
        h[4..6].concat(),
        h[6..8].concat(),
        h[8..10].concat(),
        h[10..].concat()
    )
}

/// `SetSubtypeField`: makes `field` (a short or long integer) the table's subtype field.
pub fn set_subtype_field(dir: impl AsRef<Path>, table: &str, field_name: &str) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let (name, ftype, _) = field(&t, field_name)?.clone();
        if !matches!(ftype, GdbFieldType::Int16 | GdbFieldType::Int32) {
            return Err(bad("ERROR 003911: The field is not of type SHORT | LONG"));
        }
        let mut def = t.def.clone();
        if subtype_field(&def)
            .as_deref()
            .is_some_and(|f| f.eq_ignore_ascii_case(&name))
        {
            return Ok(());
        }
        // The subtype field gets default 0 in the field list, unless it has a default already.
        let zero = coerce(&Value::BigInteger(0), ftype)?;
        let has_default = xml_defaults(&t)
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case(&name));
        def = change_field_info(
            &t,
            &def,
            &name,
            Change::Keep,
            if has_default {
                DefaultChange::Keep
            } else {
                DefaultChange::Set(&zero)
            },
        )?;
        if Document::parse(&def)
            .map_err(|e| bad(e.to_string()))?
            .descendants()
            .any(|n| n.is_element() && n.tag_name().name() == "SubtypeFieldName")
        {
            def = set_simple(&def, "SubtypeFieldName", &name);
        } else {
            let at = before_root_close(&def);
            let block = format!(
                "<SubtypeFieldName>{}</SubtypeFieldName><DefaultSubtypeCode>0</DefaultSubtypeCode><Subtypes xsi:type='typens:ArrayOfSubtype'></Subtypes>",
                xml_escape(&name)
            );
            def = apply(
                &def,
                vec![Splice {
                    at: at..at,
                    with: block,
                }],
            );
        }
        store(dir, &t, def)
    })
}

/// `AddSubtype`: a new subtype that starts with the table's field defaults.
pub fn add_subtype(dir: impl AsRef<Path>, table: &str, code: i64, name: &str) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let key = subtype_field(&t.def).ok_or_else(no_subtype_field)?;
        code_value(&t, code)?;
        if subtype_codes(&t.def).contains(&code) {
            return Err(bad("ERROR 160129: The subtype already exists."));
        }
        let mut infos = String::new();
        for (fname, v) in xml_defaults(&t) {
            if fname.eq_ignore_ascii_case(&key) {
                continue;
            }
            let ft = field(&t, &fname)?.1;
            infos += &format!(
                "<SubtypeFieldInfo xsi:type='typens:SubtypeFieldInfo'><FieldName>{}</FieldName><DefaultValue xsi:type='{}'>{}</DefaultValue></SubtypeFieldInfo>",
                xml_escape(&fname),
                xs_type(ft.esri()),
                value_text(&v).unwrap_or_default()
            );
        }
        let block = format!(
            "<Subtype xsi:type='typens:Subtype'><SubtypeName>{}</SubtypeName><SubtypeCode>{code}</SubtypeCode><FieldInfos xsi:type='typens:ArrayOfSubtypeFieldInfo'>{infos}</FieldInfos></Subtype>",
            xml_escape(name)
        );
        let doc = Document::parse(&t.def).map_err(|e| bad(e.to_string()))?;
        let list = doc
            .descendants()
            .find(|n| n.is_element() && n.tag_name().name() == "Subtypes")
            .ok_or_else(no_subtype_field)?;
        let def = apply(&t.def, vec![append_child(&t.def, list, &block)]);
        drop(doc);
        store(dir, &t, def)
    })
}

/// `RemoveSubtype`: removes the subtypes with these codes. The default code moves to the first
/// subtype left (0 when none is).
pub fn remove_subtype(dir: impl AsRef<Path>, table: &str, codes: &[i64]) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let key = subtype_field(&t.def).ok_or_else(no_subtype_field)?;
        let have = subtype_codes(&t.def);
        if codes.is_empty() || codes.iter().any(|c| !have.contains(c)) {
            return Err(invalid_code());
        }
        let doc = Document::parse(&t.def).map_err(|e| bad(e.to_string()))?;
        let edits: Vec<Splice> = doc
            .descendants()
            .filter(|n| n.is_element() && n.tag_name().name() == "Subtype")
            .filter(|n| {
                child_text(*n, "SubtypeCode")
                    .and_then(|c| c.parse::<i64>().ok())
                    .is_some_and(|c| codes.contains(&c))
            })
            .map(|n| Splice {
                at: n.range(),
                with: String::new(),
            })
            .collect();
        let mut def = apply(&t.def, edits);
        drop(doc);
        let left: Vec<i64> = have.into_iter().filter(|c| !codes.contains(c)).collect();
        let current = default_code(&def);
        if !left.contains(&current) {
            let new = left.first().copied().unwrap_or(0);
            def = set_simple(&def, "DefaultSubtypeCode", &new.to_string());
            let v = code_value(&t, new)?;
            def = change_field_info(&t, &def, &key, Change::Keep, DefaultChange::Set(&v))?;
        }
        store(dir, &t, def)
    })
}

/// `SetDefaultSubtype`.
pub fn set_default_subtype(dir: impl AsRef<Path>, table: &str, code: i64) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let key = subtype_field(&t.def).ok_or_else(no_subtype_field)?;
        let have = subtype_codes(&t.def);
        if !have.contains(&code) {
            let list: Vec<String> = have.iter().map(i64::to_string).collect();
            return Err(bad(format!(
                "ERROR 000800: The value is not a member of {}",
                list.join(" | ")
            )));
        }
        let v = code_value(&t, code)?;
        let mut def = set_simple(&t.def, "DefaultSubtypeCode", &code.to_string());
        def = change_field_info(&t, &def, &key, Change::Keep, DefaultChange::Set(&v))?;
        store(dir, &t, def)
    })
}

// ---------------------------------------------------------------------------------------------
// domains and defaults of fields

fn domain_of(dir: &Path, name: &str) -> Result<Domain> {
    crate::gdb::Geodatabase::open(dir)?
        .domain(name)
        .cloned()
        .ok_or_else(|| {
            bad(format!(
                "ERROR 000732: Domain {name} does not exist or is not supported"
            ))
        })
}

fn numeric(t: GdbFieldType) -> bool {
    matches!(
        t,
        GdbFieldType::Int16
            | GdbFieldType::Int32
            | GdbFieldType::Int64
            | GdbFieldType::Float32
            | GdbFieldType::Float64
    )
}

fn domain_fits(d: &Domain, t: GdbFieldType) -> bool {
    let domain_numeric = !matches!(
        d.field_type,
        FieldType::String | FieldType::Date | FieldType::Guid
    );
    match t {
        GdbFieldType::String => d.field_type == FieldType::String,
        GdbFieldType::DateTime => d.field_type == FieldType::Date,
        t if numeric(t) => domain_numeric,
        _ => false,
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::SmallInteger(n) => Some(f64::from(*n)),
        Value::Integer(n) => Some(f64::from(*n)),
        Value::BigInteger(n) => Some(*n as f64),
        Value::Single(n) => Some(f64::from(*n)),
        Value::Double(n) => Some(*n),
        _ => None,
    }
}

fn in_domain(d: &Domain, v: &Value) -> bool {
    match &d.kind {
        DomainKind::Range { min, max } => match (as_f64(v), as_f64(min), as_f64(max)) {
            (Some(x), Some(lo), Some(hi)) => x >= lo && x <= hi,
            _ => true,
        },
        DomainKind::Coded(values) => values.iter().any(|(c, _)| match (as_f64(c), as_f64(v)) {
            (Some(a), Some(b)) => a == b,
            _ => c == v,
        }),
    }
}

/// The `SubtypeFieldInfo` edit for `field_name` in the subtypes `codes`.
fn change_subtype_infos(
    xml: &str,
    codes: &[i64],
    field_name: &str,
    domain: Change,
    default: Option<(&Value, &str)>,
    clear_default: bool,
) -> Result<String> {
    let doc = Document::parse(xml).map_err(|e| bad(e.to_string()))?;
    let mut edits = Vec::new();
    for sub in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Subtype")
    {
        let code: i64 = child_text(sub, "SubtypeCode")
            .and_then(|c| c.parse().ok())
            .unwrap_or(i64::MIN);
        if !codes.contains(&code) {
            continue;
        }
        let Some(infos) = child(sub, "FieldInfos") else {
            continue;
        };
        let entry = children(infos, "SubtypeFieldInfo").find(|e| {
            child_text(*e, "FieldName").is_some_and(|n| n.eq_ignore_ascii_case(field_name))
        });
        let make = |dom: Option<&str>, def: Option<(&Value, &str)>| {
            let mut s = format!(
                "<SubtypeFieldInfo xsi:type='typens:SubtypeFieldInfo'><FieldName>{}</FieldName>",
                xml_escape(field_name)
            );
            if let Some(d) = dom {
                s += &format!("<DomainName>{}</DomainName>", xml_escape(d));
            }
            if let Some((v, xs)) = def {
                s += &format!(
                    "<DefaultValue xsi:type='{xs}'>{}</DefaultValue>",
                    value_text(v).unwrap_or_default()
                );
            }
            s + "</SubtypeFieldInfo>"
        };
        let old_domain = entry
            .and_then(|e| child_text(e, "DomainName"))
            .filter(|d| !d.is_empty());
        let old_default = entry
            .and_then(|e| child(e, "DefaultValue"))
            .map(|d| d.range());
        // Removing a subtype's domain drops the whole entry, as ArcGIS does.
        if matches!(domain, Change::Remove) {
            if let Some(e) = entry {
                edits.push(Splice {
                    at: e.range(),
                    with: String::new(),
                });
            }
            continue;
        }
        let new_domain: Option<String> = match domain {
            Change::Set(d) => Some(d.to_string()),
            _ => old_domain,
        };
        let new_default: Option<String> = match (default, clear_default) {
            (Some((v, xs)), _) => Some(format!(
                "<DefaultValue xsi:type='{xs}'>{}</DefaultValue>",
                value_text(v).unwrap_or_default()
            )),
            (None, true) => None,
            (None, false) => old_default.map(|r| xml[r].to_string()),
        };
        let text = if new_domain.is_none() && new_default.is_none() {
            String::new()
        } else {
            let mut s = format!(
                "<SubtypeFieldInfo xsi:type='typens:SubtypeFieldInfo'><FieldName>{}</FieldName>",
                xml_escape(field_name)
            );
            if let Some(d) = &new_domain {
                s += &format!("<DomainName>{}</DomainName>", xml_escape(d));
            }
            if let Some(d) = &new_default {
                s += d;
            }
            s + "</SubtypeFieldInfo>"
        };
        let _ = make;
        match entry {
            Some(e) => edits.push(Splice {
                at: e.range(),
                with: text,
            }),
            None if !text.is_empty() => edits.push(append_child(xml, infos, &text)),
            None => {}
        }
    }
    Ok(apply(xml, edits))
}

fn check_codes(t: &Target, codes: &[i64]) -> Result<()> {
    if codes.is_empty() {
        return Ok(());
    }
    let have = subtype_codes(&t.def);
    if subtype_field(&t.def).is_none() {
        return Err(no_subtype_field());
    }
    if codes.iter().any(|c| !have.contains(c)) {
        return Err(bad("ERROR 000192: Invalid value for Subtype"));
    }
    Ok(())
}

/// `AssignDomainToField`: with `subtype_codes`, for those subtypes; without, for the field.
pub fn assign_domain_to_field(
    dir: impl AsRef<Path>,
    table: &str,
    field_name: &str,
    domain: &str,
    subtype_codes: &[i64],
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let (fname, ftype, _) = field(&t, field_name)?.clone();
        let d = domain_of(dir, domain)?;
        if !domain_fits(&d, ftype) {
            return Err(bad("ERROR 000356: Invalid domain type."));
        }
        check_codes(&t, subtype_codes)?;
        let def = if subtype_codes.is_empty() {
            change_field_info(
                &t,
                &t.def,
                &fname,
                Change::Set(&d.name),
                DefaultChange::Keep,
            )?
        } else {
            change_subtype_infos(
                &t.def,
                subtype_codes,
                &fname,
                Change::Set(&d.name),
                None,
                false,
            )?
        };
        store(dir, &t, def)
    })
}

/// `RemoveDomainFromField`.
pub fn remove_domain_from_field(
    dir: impl AsRef<Path>,
    table: &str,
    field_name: &str,
    subtype_codes: &[i64],
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let (fname, _, _) = field(&t, field_name)?.clone();
        check_codes(&t, subtype_codes)?;
        let def = if subtype_codes.is_empty() {
            change_field_info(&t, &t.def, &fname, Change::Remove, DefaultChange::Keep)?
        } else {
            change_subtype_infos(&t.def, subtype_codes, &fname, Change::Remove, None, false)?
        };
        store(dir, &t, def)
    })
}

/// `AssignDefaultToField`: sets (or with `None`, clears) the default of the field, or of the
/// field within the subtypes `subtype_codes`. A field's default is also written to the table.
pub fn assign_default_to_field(
    dir: impl AsRef<Path>,
    table: &str,
    field_name: &str,
    value: Option<&Value>,
    subtype_codes: &[i64],
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let t = load_target(dir, table)?;
        let (fname, ftype, _) = field(&t, field_name)?.clone();
        check_codes(&t, subtype_codes)?;
        let v = value.map(|v| coerce(v, ftype)).transpose()?;
        // The default must be a value the field's domain allows.
        let doc = Document::parse(&t.def).map_err(|e| bad(e.to_string()))?;
        let domain_name: Option<String> = if subtype_codes.is_empty() {
            doc.descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "GPFieldInfoEx")
                .find(|e| child_text(*e, "Name").is_some_and(|n| n.eq_ignore_ascii_case(&fname)))
                .and_then(|e| child_text(e, "DomainName"))
        } else {
            doc.descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "Subtype")
                .filter(|n| {
                    child_text(*n, "SubtypeCode")
                        .and_then(|c| c.parse::<i64>().ok())
                        .is_some_and(|c| subtype_codes.contains(&c))
                })
                .flat_map(|s| {
                    s.descendants()
                        .filter(|n| n.is_element() && n.tag_name().name() == "SubtypeFieldInfo")
                        .collect::<Vec<_>>()
                })
                .find(|e| {
                    child_text(*e, "FieldName").is_some_and(|n| n.eq_ignore_ascii_case(&fname))
                })
                .and_then(|e| child_text(e, "DomainName"))
        }
        .filter(|d| !d.is_empty());
        drop(doc);
        if let (Some(d), Some(v)) = (domain_name, &v) {
            let dom = domain_of(dir, &d)?;
            if !in_domain(&dom, v) {
                return Err(bad(
                    "ERROR 160080: Default value is invalid within the domain.",
                ));
            }
        }
        if !subtype_codes.is_empty() {
            let xs = xs_type(ftype.esri());
            let def = change_subtype_infos(
                &t.def,
                subtype_codes,
                &fname,
                Change::Keep,
                v.as_ref().map(|v| (v, xs)),
                v.is_none(),
            )?;
            return store(dir, &t, def);
        }
        let def = change_field_info(
            &t,
            &t.def,
            &fname,
            Change::Keep,
            match &v {
                Some(v) => DefaultChange::Set(v),
                None => DefaultChange::Remove,
            },
        )?;
        store(dir, &t, def.clone())?;
        // The table's own field definitions take the defaults the definition now gives.
        let t = load_target(dir, table)?;
        let defaults = xml_defaults(&t);
        let table_file = Table::open(base(dir, t.table_id))?;
        let mut rows = Vec::new();
        for r in table_file.features() {
            let f = r?;
            rows.push((Some(f.oid as u64), f));
        }
        let fields: Vec<unigis_core::Field> = table_file
            .fields()
            .iter()
            .map(|f| {
                let mut f = f.clone();
                f.default = defaults
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(&f.name))
                    .map(|(_, v)| v.clone());
                f
            })
            .collect();
        rewrite_table(base(dir, t.table_id), &fields, &rows)?;
        let _ = decode_default;
        Ok(())
    })
}
