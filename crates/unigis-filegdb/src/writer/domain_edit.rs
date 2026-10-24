//! Creating and editing attribute domains of an existing geodatabase (`arcpy.management.
//! CreateDomain`, `AddCodedValueToDomain`, `DeleteCodedValueFromDomain`,
//! `AlterCodedValueDescription`, `SetValueForRangeDomain`, `SortCodedValueDomain`, `AlterDomain`,
//! `TableToDomain`).
//!
//! As observed through the bridge (`testdata/gdb-reference/domain_ops`):
//! - A new coded domain has no values; a new range domain has the minimum and maximum 0 (for dates
//!   1899-12-30). A name with spaces keeps them in the item, the definition's `DomainName` loses
//!   them. An existing name is error 000111.
//! - Adding a code that exists changes its description in place; adding to a range domain or a
//!   missing domain is error 000800; a code that does not fit the domain's type is error 000378.
//! - Deleting a code that is not there is error 000192; changing the description of one is error
//!   000800 (listing the codes). Setting a range on a coded domain is error 000375; the bounds are
//!   not checked against each other.
//! - Sorting reorders the values in the definition.
//! - Altering a domain's name also renames it in the fields that use it; its description, split
//!   and merge policy and owner are kept in the definition.
//! - `TableToDomain` makes a coded domain (the code field's type decides its type) or, for an
//!   existing domain, appends (a code with another description is error 000980) or replaces
//!   its values.

use std::path::Path;

use roxmltree::Document;
use unigis_core::{Feature, FieldType, Result, Value};

use super::catalog::{Kind, base, find, load_items, missing, text};
use super::edit::TableEditor;
use super::subtype_edit::bad;
use super::{GeodatabaseWriter, domain_xml};
use crate::schema::{Domain, DomainKind, MergePolicy, SplitPolicy, parse_domain_xml};

/// The type name ArcGIS uses in "Invalid X type" messages.
fn type_label(t: FieldType) -> &'static str {
    match t {
        FieldType::SmallInteger => "Short",
        FieldType::Integer => "Long",
        FieldType::BigInteger => "Big Integer",
        FieldType::Single => "Float",
        FieldType::Double => "Double",
        FieldType::Date => "Date",
        _ => "Text",
    }
}

/// `raw` as a value of the domain's field type (error 000378 when it does not fit).
pub fn parse_domain_value(raw: &str, t: FieldType) -> Result<Value> {
    let raw = raw.trim();
    let invalid = || bad(format!("ERROR 000378: Invalid {} type.", type_label(t)));
    Ok(match t {
        FieldType::SmallInteger => Value::SmallInteger(raw.parse().map_err(|_| invalid())?),
        FieldType::Integer => Value::Integer(raw.parse().map_err(|_| invalid())?),
        FieldType::BigInteger => Value::BigInteger(raw.parse().map_err(|_| invalid())?),
        FieldType::Single => Value::Single(raw.parse().map_err(|_| invalid())?),
        FieldType::Double => Value::Double(raw.parse().map_err(|_| invalid())?),
        FieldType::Date => Value::Date(
            chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%dT%H:%M:%S%.f")
                .or_else(|_| chrono::NaiveDateTime::parse_from_str(raw, "%Y-%m-%d %H:%M:%S%.f"))
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                })
                .map_err(|_| invalid())?,
        ),
        _ => Value::String(raw.to_owned()),
    })
}

fn zero(t: FieldType) -> Value {
    match t {
        FieldType::SmallInteger => Value::SmallInteger(0),
        FieldType::Integer => Value::Integer(0),
        FieldType::BigInteger => Value::BigInteger(0),
        FieldType::Single => Value::Single(0.0),
        FieldType::Double => Value::Double(0.0),
        FieldType::Date => Value::Date(
            chrono::NaiveDate::from_ymd_opt(1899, 12, 30)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        ),
        _ => Value::String(String::new()),
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

fn same(a: &Value, b: &Value) -> bool {
    match (as_f64(a), as_f64(b)) {
        (Some(x), Some(y)) => x == y,
        _ => a == b,
    }
}

fn shown(v: &Value) -> String {
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

/// A domain as stored: its item row, the parsed domain and its owner.
struct Stored {
    oid: u64,
    row: Feature,
    c_def: usize,
    domain: Domain,
    owner: String,
}

fn load(dir: &Path, name: &str) -> Result<Stored> {
    let items = load_items(dir)?;
    let item = find(&items, name, |k| k == Kind::Domain).ok_or_else(|| missing(name))?;
    let xml = text(&item.row.attributes[items.c_def]);
    let domain =
        parse_domain_xml(&xml).ok_or_else(|| bad(format!("domain {name} is unreadable")))?;
    let owner = Document::parse(&xml)
        .ok()
        .and_then(|d| {
            d.descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "Owner")
                .and_then(|n| n.text().map(str::to_owned))
        })
        .unwrap_or_default();
    Ok(Stored {
        oid: item.row.oid as u64,
        row: item.row.clone(),
        c_def: items.c_def,
        domain,
        owner,
    })
}

fn store(dir: &Path, mut s: Stored) -> Result<()> {
    let mut xml = domain_xml(&s.domain);
    if !s.owner.is_empty() {
        xml = xml.replace(
            "<Owner></Owner>",
            &format!("<Owner>{}</Owner>", super::xml_escape(&s.owner)),
        );
    }
    s.row.attributes[s.c_def] = Value::Xml(xml);
    let mut ed = TableEditor::open(base(dir, 4))?;
    ed.update(s.oid, &s.row)?;
    ed.finish().map(|_| ())
}

fn not_a_member(list: impl Iterator<Item = String>) -> unigis_core::Error {
    bad(format!(
        "ERROR 000800: The value is not a member of {}.",
        list.collect::<Vec<_>>().join(" | ")
    ))
}

/// `CreateDomain`: an empty coded domain, or a range domain from 0 to 0.
pub fn create_domain(
    dir: impl AsRef<Path>,
    name: &str,
    description: &str,
    field_type: FieldType,
    range: bool,
    split_policy: SplitPolicy,
    merge_policy: MergePolicy,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut w = GeodatabaseWriter::open_existing(dir)?;
        if w.has_domain(name) {
            return Err(bad("ERROR 000111: Domain already exists."));
        }
        let domain = Domain {
            name: name.to_owned(),
            description: description.to_owned(),
            field_type,
            split_policy,
            merge_policy,
            kind: if range {
                DomainKind::Range {
                    min: zero(field_type),
                    max: zero(field_type),
                }
            } else {
                DomainKind::Coded(Vec::new())
            },
        };
        w.add_domain(&domain)?;
        w.finish()?;
        if name.contains(' ') {
            // The definition names the domain without the spaces.
            let mut s = load(dir, name)?;
            s.domain.name = name.replace(' ', "");
            store(dir, s)?;
        }
        Ok(())
    })
}

/// The values of a coded domain as (code, description), or a range domain's bounds as the rows
/// `Min value` and `Max value` (`DomainToTable`), with the domain's field type.
pub fn domain_rows(dir: impl AsRef<Path>, name: &str) -> Result<(FieldType, Vec<(Value, String)>)> {
    let s = load(dir.as_ref(), name)?;
    let rows = match s.domain.kind {
        DomainKind::Coded(values) => values,
        DomainKind::Range { min, max } => {
            vec![(min, "Min value".to_owned()), (max, "Max value".to_owned())]
        }
    };
    Ok((s.domain.field_type, rows))
}

fn coded(s: &mut Stored) -> Result<&mut Vec<(Value, String)>> {
    match &mut s.domain.kind {
        DomainKind::Coded(values) => Ok(values),
        DomainKind::Range { .. } => Err(not_a_member(std::iter::empty())),
    }
}

/// `AddCodedValueToDomain`: a new code goes last, an existing one gets the description.
pub fn add_coded_value(
    dir: impl AsRef<Path>,
    name: &str,
    code: &str,
    description: &str,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut s = load(dir, name).map_err(|_| coded_domain_names(dir))?;
        let code = parse_domain_value(code, s.domain.field_type)?;
        let values = coded(&mut s).map_err(|_| coded_domain_names(dir))?;
        match values.iter_mut().find(|(c, _)| same(c, &code)) {
            Some(v) => v.1 = description.to_owned(),
            None => values.push((code, description.to_owned())),
        }
        store(dir, s)
    })
}

/// The error for a domain that is not a coded domain: the coded domains are the choices.
fn coded_domain_names(dir: &Path) -> unigis_core::Error {
    let names = load_items(dir)
        .map(|items| {
            items
                .rows
                .iter()
                .filter(|i| i.kind == Kind::Domain)
                .filter_map(|i| {
                    let xml = text(&i.row.attributes[items.c_def]);
                    xml.contains("GPCodedValueDomain2").then(|| i.name.clone())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    not_a_member(names.into_iter())
}

/// `DeleteCodedValueFromDomain`: codes separated by `;`.
pub fn delete_coded_values(dir: impl AsRef<Path>, name: &str, codes: &str) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut s = load(dir, name).map_err(|_| coded_domain_names(dir))?;
        let t = s.domain.field_type;
        let wanted: Vec<Value> = codes
            .split(';')
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(|c| parse_domain_value(c, t))
            .collect::<Result<_>>()?;
        let values = coded(&mut s).map_err(|_| coded_domain_names(dir))?;
        if wanted
            .iter()
            .any(|w| !values.iter().any(|(c, _)| same(c, w)))
        {
            return Err(bad("ERROR 000192: Invalid value for Code Value"));
        }
        values.retain(|(c, _)| !wanted.iter().any(|w| same(c, w)));
        store(dir, s)
    })
}

/// `AlterCodedValueDescription`.
pub fn alter_coded_value_description(
    dir: impl AsRef<Path>,
    name: &str,
    code: &str,
    description: &str,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut s = load(dir, name).map_err(|_| coded_domain_names(dir))?;
        let code = parse_domain_value(code, s.domain.field_type)?;
        let values = coded(&mut s).map_err(|_| coded_domain_names(dir))?;
        match values.iter_mut().find(|(c, _)| same(c, &code)) {
            Some(v) => v.1 = description.to_owned(),
            None => return Err(not_a_member(values.iter().map(|(c, _)| shown(c)))),
        }
        store(dir, s)
    })
}

/// `SetValueForRangeDomain`.
pub fn set_value_for_range(dir: impl AsRef<Path>, name: &str, min: &str, max: &str) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut s = load(dir, name)?;
        let t = s.domain.field_type;
        let (lo, hi) = (parse_domain_value(min, t)?, parse_domain_value(max, t)?);
        match &mut s.domain.kind {
            DomainKind::Range { min, max } => {
                *min = lo;
                *max = hi;
            }
            DomainKind::Coded(_) => return Err(bad("ERROR 000375: Invalid range domain.")),
        }
        store(dir, s)
    })
}

/// `SortCodedValueDomain`: by code or description, ascending or descending.
pub fn sort_coded_values(
    dir: impl AsRef<Path>,
    name: &str,
    by_code: bool,
    ascending: bool,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let mut s = load(dir, name).map_err(|_| coded_domain_names(dir))?;
        let values = coded(&mut s).map_err(|_| coded_domain_names(dir))?;
        values.sort_by(|a, b| {
            let o = if by_code {
                match (as_f64(&a.0), as_f64(&b.0)) {
                    (Some(x), Some(y)) => x.total_cmp(&y),
                    _ => shown(&a.0).cmp(&shown(&b.0)),
                }
            } else {
                a.1.to_lowercase().cmp(&b.1.to_lowercase())
            };
            if ascending { o } else { o.reverse() }
        });
        store(dir, s)
    })
}

/// `AlterDomain`: a new name (also in the fields that use the domain), description, policies or
/// owner.
pub fn alter_domain(
    dir: impl AsRef<Path>,
    name: &str,
    new_name: Option<&str>,
    description: Option<&str>,
    split_policy: Option<SplitPolicy>,
    merge_policy: Option<MergePolicy>,
    owner: Option<&str>,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        load(dir, name)?;
        let mut current = name.to_owned();
        if let Some(new) = new_name.filter(|n| !n.is_empty() && *n != name) {
            super::rename_domain(dir, name, new)?;
            current = new.to_owned();
        }
        if description.is_none()
            && split_policy.is_none()
            && merge_policy.is_none()
            && owner.is_none()
        {
            return Ok(());
        }
        let mut s = load(dir, &current)?;
        if let Some(d) = description {
            s.domain.description = d.to_owned();
        }
        if let Some(p) = split_policy {
            s.domain.split_policy = p;
        }
        if let Some(p) = merge_policy {
            s.domain.merge_policy = p;
        }
        if let Some(o) = owner {
            s.owner = o.to_owned();
        }
        store(dir, s)
    })
}

/// `TableToDomain`: the values (code, description) of a table as a coded domain of the code
/// field's type. A new domain is created; an existing one gets the values appended (a code with
/// another description is error 000980) or replaced.
pub fn table_to_domain(
    dir: impl AsRef<Path>,
    name: &str,
    description: &str,
    field_type: FieldType,
    values: &[(Value, String)],
    replace: bool,
) -> Result<()> {
    let dir = dir.as_ref();
    super::txn::atomic(dir, || -> Result<()> {
        let exists = load_items(dir)
            .map(|items| find(&items, name, |k| k == Kind::Domain).is_some())
            .unwrap_or(false);
        if !exists {
            create_domain(
                dir,
                name,
                description,
                field_type,
                false,
                SplitPolicy::DefaultValue,
                MergePolicy::DefaultValue,
            )?;
        }
        let mut s = load(dir, name)?;
        let current = coded(&mut s).map_err(|_| coded_domain_names(dir))?;
        if replace {
            current.clear();
        }
        for (code, desc) in values {
            match current.iter().find(|(c, _)| same(c, code)) {
                Some((_, old)) if old != desc => {
                    return Err(bad(format!(
                        "ERROR 000980: Domain value {} description {desc} already exists as description {old}.",
                        shown(code)
                    )));
                }
                Some(_) => {}
                None => current.push((code.clone(), desc.clone())),
            }
        }
        store(dir, s)
    })
}
