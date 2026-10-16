//! Deleting and renaming items of an existing geodatabase (`arcpy.management.Delete`,
//! `Rename`, `DeleteDomain`, `AlterDomain`).
//!
//! What ArcGIS does, observed through the bridge (`docs/formats/filegdb.md`):
//! - **Delete** removes the item's row from `GDB_Items`, the `GDB_ItemRelationships` rows that
//!   mention it, its row in `GDB_SystemCatalog` (the OBJECTID is left as a gap) and the table's
//!   files. Deleting a table or feature class also deletes the relationship classes that use it;
//!   deleting a feature dataset deletes the feature classes in it. `GDB_SpatialRefs` is untouched.
//! - **Rename** changes `Name`, `PhysicalName` (upper case) and `Path` of the item (and the paths
//!   of a feature dataset's children), the catalog name, and the names inside the definition XML:
//!   `CatalogPath`, `Name`, an empty table-level `AliasName` (which becomes the new name), the
//!   class names of relationship classes that use the item, `DomainName` for domains. The item's
//!   documentation and item information are left as they are.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use geoforge_core::{Error, Feature, Result, Value};

use super::edit::TableEditor;
use super::{TYPE_CODED_DOMAIN, TYPE_RANGE_DOMAIN, validate_table_name};
use crate::table::Table;

const T_FEATURE_CLASS: &str = "{70737809-";
const T_TABLE: &str = "{CD06BC3B-";
const T_DATASET: &str = "{74737149-";
const T_RELATIONSHIP_CLASS: &str = "{B606A7E1-";
/// `DatasetsRelatedThrough`: a relationship class's link to each class it relates.
const REL_RELATED_THROUGH: &str = "{725BADAB-";
const REL_DOMAIN_IN_DATASET: &str = "{17E08ADB-";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    FeatureClass,
    Table,
    Dataset,
    RelationshipClass,
    Domain,
    Other,
}

fn kind_of(ty: &str) -> Kind {
    let is = |p: &str| ty.len() >= p.len() && ty[..p.len()].eq_ignore_ascii_case(p);
    if is(T_FEATURE_CLASS) {
        Kind::FeatureClass
    } else if is(T_TABLE) {
        Kind::Table
    } else if is(T_DATASET) {
        Kind::Dataset
    } else if is(T_RELATIONSHIP_CLASS) {
        Kind::RelationshipClass
    } else if is(&TYPE_CODED_DOMAIN[..10]) || is(&TYPE_RANGE_DOMAIN[..10]) {
        Kind::Domain
    } else {
        Kind::Other
    }
}

/// A `GDB_Items` row with the columns the edits need located.
struct ItemRow {
    row: Feature,
    uuid: String,
    kind: Kind,
    name: String,
    path: String,
}

struct Items {
    rows: Vec<ItemRow>,
    c_name: usize,
    c_physical: usize,
    c_path: usize,
    c_def: usize,
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) | Value::Guid(s) | Value::Xml(s) => s.clone(),
        _ => String::new(),
    }
}

fn column(t: &Table, name: &str) -> Result<usize> {
    t.fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::Unsupported(format!("{} has no {name} field", "a system table")))
}

fn base(dir: &Path, id: u64) -> PathBuf {
    dir.join(format!("a{id:08x}"))
}

fn load_items(dir: &Path) -> Result<Items> {
    let t = Table::open(base(dir, 4))?;
    let (c_uuid, c_type) = (column(&t, "UUID")?, column(&t, "Type")?);
    let (c_name, c_physical, c_path, c_def) = (
        column(&t, "Name")?,
        column(&t, "PhysicalName")?,
        column(&t, "Path")?,
        column(&t, "Definition")?,
    );
    let mut rows = Vec::new();
    for r in t.features() {
        let row = r?;
        rows.push(ItemRow {
            uuid: text(&row.attributes[c_uuid]),
            kind: kind_of(&text(&row.attributes[c_type])),
            name: text(&row.attributes[c_name]),
            path: text(&row.attributes[c_path]),
            row,
        });
    }
    Ok(Items {
        rows,
        c_name,
        c_physical,
        c_path,
        c_def,
    })
}

/// ArcGIS's error for a name nothing answers to.
fn missing(name: &str) -> Error {
    Error::Unsupported(format!(
        "ERROR 000732: Dataset {name} does not exist or is not supported"
    ))
}

fn is_class(k: Kind) -> bool {
    matches!(
        k,
        Kind::FeatureClass | Kind::Table | Kind::RelationshipClass | Kind::Dataset
    )
}

fn find<'a>(items: &'a Items, name: &str, want: impl Fn(Kind) -> bool) -> Option<&'a ItemRow> {
    items
        .rows
        .iter()
        .find(|i| want(i.kind) && i.name.eq_ignore_ascii_case(name))
}

/// Deletes a table, feature class, relationship class or feature dataset by name, with what
/// goes with it (see the module notes). Returns the names of the items removed.
pub fn delete_item(dir: impl AsRef<Path>, name: &str) -> Result<Vec<String>> {
    let dir = dir.as_ref();
    let items = load_items(dir)?;
    let target = find(&items, name, is_class).ok_or_else(|| missing(name))?;

    let rels = Table::open(base(dir, 6))?;
    let (r_origin, r_dest, r_type) = (
        column(&rels, "OriginID")?,
        column(&rels, "DestID")?,
        column(&rels, "Type")?,
    );
    let rel_rows: Vec<Feature> = rels.features().collect::<Result<_>>()?;

    // The items that go: the target, a dataset's contents, and the relationship classes that
    // relate any of them.
    let mut doomed: HashSet<String> = HashSet::new();
    doomed.insert(target.uuid.clone());
    if target.kind == Kind::Dataset {
        let prefix = format!("{}\\", target.path);
        for i in items.rows.iter().filter(|i| {
            i.path.len() > prefix.len() && i.path[..prefix.len()].eq_ignore_ascii_case(&prefix)
        }) {
            doomed.insert(i.uuid.clone());
        }
    }
    for r in &rel_rows {
        let ty = text(&r.attributes[r_type]);
        let origin = text(&r.attributes[r_origin]);
        if ty.len() >= REL_RELATED_THROUGH.len()
            && ty[..REL_RELATED_THROUGH.len()].eq_ignore_ascii_case(REL_RELATED_THROUGH)
            && doomed.contains(&origin)
        {
            doomed.insert(text(&r.attributes[r_dest]));
        }
    }
    let gone: Vec<&ItemRow> = items
        .rows
        .iter()
        .filter(|i| doomed.contains(&i.uuid))
        .collect();

    // The catalog rows of the tables among them.
    let catalog = Table::open(base(dir, 1))?;
    let mut tables: Vec<(u64, String)> = Vec::new();
    for r in catalog.features() {
        let r = r?;
        let n = text(&r.attributes[0]);
        if gone
            .iter()
            .any(|i| i.kind != Kind::Dataset && i.name.eq_ignore_ascii_case(&n))
        {
            tables.push((r.oid as u64, n));
        }
    }

    // Unlist the tables first, so nothing lists a table whose item is gone.
    if !tables.is_empty() {
        let mut ed = TableEditor::open(base(dir, 1))?;
        for (oid, _) in &tables {
            ed.delete(*oid)?;
        }
        ed.finish()?;
    }
    let mut ed = TableEditor::open(base(dir, 6))?;
    for r in &rel_rows {
        if doomed.contains(&text(&r.attributes[r_origin]))
            || doomed.contains(&text(&r.attributes[r_dest]))
        {
            ed.delete(r.row_oid())?;
        }
    }
    ed.finish()?;
    let mut ed = TableEditor::open(base(dir, 4))?;
    for i in &gone {
        ed.delete(i.row.oid as u64)?;
    }
    ed.finish()?;
    for (id, _) in &tables {
        remove_files(dir, *id)?;
    }
    Ok(gone.iter().map(|i| i.name.clone()).collect())
}

trait RowOid {
    fn row_oid(&self) -> u64;
}

impl RowOid for Feature {
    fn row_oid(&self) -> u64 {
        self.oid as u64
    }
}

/// Removes every file of table number `id` (`aXXXXXXXX.*`).
fn remove_files(dir: &Path, id: u64) -> Result<()> {
    let prefix = format!("a{id:08x}.");
    let entries = std::fs::read_dir(dir).map_err(|e| Error::io(dir, e))?;
    for e in entries.flatten() {
        if e.file_name()
            .to_string_lossy()
            .to_ascii_lowercase()
            .starts_with(&prefix)
        {
            std::fs::remove_file(e.path()).map_err(|er| Error::io(e.path(), er))?;
        }
    }
    Ok(())
}

/// Replaces the first `<tag>from</tag>` at or after byte `at` of `xml` with `<tag>to</tag>`.
fn replace_tag(xml: &str, at: usize, tag: &str, from: &str, to: &str) -> Option<String> {
    let needle = format!("<{tag}>{from}</{tag}>");
    let i = at + xml.get(at..)?.find(&needle)?;
    Some(format!(
        "{}<{tag}>{to}</{tag}>{}",
        &xml[..i],
        &xml[i + needle.len()..]
    ))
}

/// Replaces every `<tag>from</tag>` in `xml`.
fn replace_all_tags(xml: &str, tag: &str, from: &str, to: &str) -> String {
    xml.replace(
        &format!("<{tag}>{from}</{tag}>"),
        &format!("<{tag}>{to}</{tag}>"),
    )
}

/// A class's definition after it is renamed from `old` (at `old_path`) to `new`.
fn renamed_definition(
    def: &str,
    old: &str,
    old_path: &str,
    new: &str,
    new_path: &str,
    alias: bool,
) -> String {
    let mut d = def.to_string();
    if let Some(i) = d.find(&format!("<CatalogPath>{old_path}</CatalogPath>")) {
        if let Some(x) = replace_tag(&d, i, "CatalogPath", old_path, new_path) {
            d = x;
        }
        if let Some(x) = replace_tag(&d, i, "Name", old, new) {
            d = x;
        }
    }
    if alias {
        // The table-level alias follows the field list; an empty one takes the new name.
        let from = d.find("</GPFieldInfoExs>").unwrap_or(0);
        if let Some(a) = d[from..].find("<AliasName>").map(|a| a + from)
            && let Some(b) = d[a..].find("</AliasName>").map(|b| b + a)
            && d[a + 11..b].trim().is_empty()
        {
            d = format!("{}<AliasName>{new}</AliasName>{}", &d[..a], &d[b + 12..]);
        }
    }
    d
}

/// Renames a table, feature class, relationship class or feature dataset
/// (`arcpy.management.Rename`).
pub fn rename_item(dir: impl AsRef<Path>, old: &str, new: &str) -> Result<()> {
    let dir = dir.as_ref();
    validate_table_name(new)?;
    let items = load_items(dir)?;
    let target = find(&items, old, is_class).ok_or_else(|| missing(old))?;
    if find(&items, new, is_class).is_some() {
        return Err(Error::Unsupported(format!(
            "ERROR 000725: Output Dataset or Feature Class: Dataset {new} already exists."
        )));
    }
    let (old_name, old_path, kind) = (target.name.clone(), target.path.clone(), target.kind);
    let new_path = match old_path.rfind('\\') {
        Some(i) => format!("{}{new}", &old_path[..=i]),
        None => new.to_string(),
    };

    // Catalog first: a table of the new name must not be listed before its item says so.
    let catalog = Table::open(base(dir, 1))?;
    let mut ed = TableEditor::open(base(dir, 1))?;
    for r in catalog.features() {
        let mut r = r?;
        if kind != Kind::Dataset && text(&r.attributes[0]).eq_ignore_ascii_case(&old_name) {
            r.attributes[0] = Value::String(new.to_string());
            ed.update(r.oid as u64, &r)?;
        }
    }
    ed.finish()?;

    let mut ed = TableEditor::open(base(dir, 4))?;
    let child_prefix = format!("{old_path}\\");
    for it in &items.rows {
        let mut row = it.row.clone();
        let mut changed = false;
        if it.uuid == target.uuid {
            row.attributes[items.c_name] = Value::String(new.to_string());
            row.attributes[items.c_physical] = Value::String(new.to_uppercase());
            row.attributes[items.c_path] = Value::String(new_path.clone());
            let alias = matches!(kind, Kind::FeatureClass | Kind::Table);
            let def = renamed_definition(
                &text(&it.row.attributes[items.c_def]),
                &old_name,
                &old_path,
                new,
                &new_path,
                alias,
            );
            row.attributes[items.c_def] = Value::Xml(def);
            changed = true;
        } else if kind == Kind::Dataset
            && it.path.len() > child_prefix.len()
            && it.path[..child_prefix.len()].eq_ignore_ascii_case(&child_prefix)
        {
            // A feature dataset's children move with it.
            let moved = format!("{new_path}{}", &it.path[old_path.len()..]);
            row.attributes[items.c_path] = Value::String(moved.clone());
            let def = text(&it.row.attributes[items.c_def]).replace(
                &format!("<CatalogPath>{}</CatalogPath>", it.path),
                &format!("<CatalogPath>{moved}</CatalogPath>"),
            );
            row.attributes[items.c_def] = Value::Xml(def);
            changed = true;
        } else if it.kind == Kind::RelationshipClass
            && matches!(kind, Kind::FeatureClass | Kind::Table)
        {
            // Relationship classes name the classes they relate.
            let def = text(&it.row.attributes[items.c_def]);
            let new_def = ["OriginClassNames", "DestinationClassNames"]
                .iter()
                .fold(def.clone(), |d, block| {
                    rename_in_block(&d, block, &old_name, new)
                });
            if new_def != def {
                row.attributes[items.c_def] = Value::Xml(new_def);
                changed = true;
            }
        }
        if changed {
            ed.update(it.row.oid as u64, &row)?;
        }
    }
    ed.finish().map(|_| ())
}

/// Replaces `<Name>from</Name>` inside the element `block`.
fn rename_in_block(xml: &str, block: &str, from: &str, to: &str) -> String {
    let Some(a) = xml
        .find(&format!("<{block} "))
        .or_else(|| xml.find(&format!("<{block}>")))
    else {
        return xml.to_string();
    };
    let Some(b) = xml[a..].find(&format!("</{block}>")).map(|b| b + a) else {
        return xml.to_string();
    };
    format!(
        "{}{}{}",
        &xml[..a],
        replace_all_tags(&xml[a..b], "Name", from, to),
        &xml[b..]
    )
}

/// Renames a domain (`arcpy.management.AlterDomain`): the item, and the fields of every table
/// that use it.
pub fn rename_domain(dir: impl AsRef<Path>, old: &str, new: &str) -> Result<()> {
    let dir = dir.as_ref();
    validate_table_name(new)?;
    let items = load_items(dir)?;
    let target = find(&items, old, |k| k == Kind::Domain).ok_or_else(|| missing(old))?;
    if find(&items, new, |k| k == Kind::Domain).is_some() {
        return Err(Error::Unsupported(format!(
            "ERROR 000725: Domain {new} already exists."
        )));
    }
    let old_name = target.name.clone();
    let mut ed = TableEditor::open(base(dir, 4))?;
    for it in &items.rows {
        let mut row = it.row.clone();
        let def = text(&it.row.attributes[items.c_def]);
        if it.uuid == target.uuid {
            row.attributes[items.c_name] = Value::String(new.to_string());
            row.attributes[items.c_physical] = Value::String(new.to_uppercase());
        } else if !matches!(it.kind, Kind::FeatureClass | Kind::Table) {
            continue;
        }
        let new_def = replace_all_tags(&def, "DomainName", &old_name, new);
        if new_def == def && it.uuid != target.uuid {
            continue;
        }
        row.attributes[items.c_def] = Value::Xml(new_def);
        ed.update(it.row.oid as u64, &row)?;
    }
    ed.finish().map(|_| ())
}

/// Deletes a domain (`arcpy.management.DeleteDomain`); one that fields still use is refused, as
/// in ArcGIS.
pub fn delete_domain(dir: impl AsRef<Path>, name: &str) -> Result<()> {
    let dir = dir.as_ref();
    let items = load_items(dir)?;
    let target = find(&items, name, |k| k == Kind::Domain).ok_or_else(|| missing(name))?;
    let rels = Table::open(base(dir, 6))?;
    let (r_dest, r_type) = (column(&rels, "DestID")?, column(&rels, "Type")?);
    let rel_rows: Vec<Feature> = rels.features().collect::<Result<_>>()?;
    let uses = |r: &Feature| {
        let ty = text(&r.attributes[r_type]);
        ty.len() >= REL_DOMAIN_IN_DATASET.len()
            && ty[..REL_DOMAIN_IN_DATASET.len()].eq_ignore_ascii_case(REL_DOMAIN_IN_DATASET)
            && text(&r.attributes[r_dest]) == target.uuid
    };
    if rel_rows.iter().any(uses) {
        return Err(Error::Unsupported(
            "ERROR 002630: The domain is being used as a default domain.".into(),
        ));
    }
    let mut ed = TableEditor::open(base(dir, 6))?;
    let (r_origin, _) = (column(&rels, "OriginID")?, ());
    for r in &rel_rows {
        if text(&r.attributes[r_origin]) == target.uuid
            || text(&r.attributes[r_dest]) == target.uuid
        {
            ed.delete(r.row_oid())?;
        }
    }
    ed.finish()?;
    let mut ed = TableEditor::open(base(dir, 4))?;
    ed.delete(target.row.oid as u64)?;
    ed.finish()?;
    Ok(())
}
