//! FileGDB extraction of simple, standalone classes/tables and key relationships.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use unigis_core::{Error, Feature, Result, Value};

use super::{GeodatabaseWriter, TableBuilder, table::TableWriter};
use crate::{Cardinality, Geodatabase, RelationshipClass, Table};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expand {
    Defaults,
    SchemaOnly,
    AllRows,
    None,
}
#[derive(Debug, Clone, Copy)]
pub struct ExtractOptions {
    pub schema_only: bool,
    pub expand: Expand,
    pub related: bool,
    pub all_table_rows: bool,
}

struct Selected {
    table: Table,
    rows: Vec<Feature>,
    selected: BTreeSet<usize>,
}
fn bad(message: impl Into<String>) -> Error {
    Error::Unsupported(message.into())
}

/// Extracts from one source FileGDB to a new or existing FileGDB. Inputs and
/// collisions are preflighted; existing outputs use the normal rollback journal.
/// New outputs are removed on failure. Feature datasets and advanced relationships
/// are refused rather than silently flattened or stripped.
pub fn extract_data(
    source: &Path,
    names: &[String],
    destination: &Path,
    create_new: bool,
    options: ExtractOptions,
) -> Result<()> {
    if names.is_empty() {
        return Err(bad("ERROR 000735: Datasets to Extract: Value is required"));
    }
    let src = Geodatabase::open(source)?;
    if destination.exists()
        && std::fs::canonicalize(source).ok() == std::fs::canonicalize(destination).ok()
    {
        return Err(bad(
            "extract destination must differ from its source geodatabase",
        ));
    }
    if create_new && destination.exists() {
        return Err(bad("ERROR 000258: Output geodatabase already exists"));
    }
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut explicit = BTreeSet::new();
    for name in names {
        let entry = src
            .entries()
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                bad(format!(
                    "ERROR 000732: Dataset {name} does not exist or is not supported"
                ))
            })?;
        wanted.insert(entry.name.clone());
        explicit.insert(entry.name.clone());
    }
    if options.expand != Expand::None {
        loop {
            let before = wanted.len();
            for r in src.relationship_classes() {
                if wanted.contains(&r.origin) || wanted.contains(&r.destination) {
                    wanted.insert(r.origin.clone());
                    wanted.insert(r.destination.clone());
                }
            }
            if wanted.len() == before {
                break;
            }
        }
    }
    let relationships: Vec<_> = src
        .relationship_classes()
        .iter()
        .filter(|r| {
            options.expand != Expand::None
                && wanted.contains(&r.origin)
                && wanted.contains(&r.destination)
        })
        .collect();
    if relationships.iter().any(|r| {
        r.is_attributed
            || r.cardinality == Cardinality::ManyToMany
            || r.origin_primary_key.eq_ignore_ascii_case("OBJECTID")
    }) {
        return Err(bad(
            "extraction of attributed, many-to-many or OBJECTID-key relationships is not supported",
        ));
    }
    let items = super::catalog::load_items(source)?;
    let mut selected = BTreeMap::new();
    for name in wanted {
        let entry = src
            .entries()
            .iter()
            .find(|e| e.name == name)
            .ok_or_else(|| bad("missing expanded dataset"))?;
        if entry.feature_dataset.is_some() {
            return Err(bad("extraction of feature datasets is not supported"));
        }
        if let Some(item) = items.rows.iter().find(|i| i.name == name) {
            let xml = super::catalog::text(&item.row.attributes[items.c_def]);
            if let Ok(doc) = roxmltree::Document::parse(&xml)
                && doc.descendants().any(|n| {
                    n.is_element()
                        && (matches!(
                            n.tag_name().name(),
                            "EditorTrackingEnabled" | "IsCOGOEnabled" | "HasAttachments"
                        ) && n.text().is_some_and(|t| t.eq_ignore_ascii_case("true")))
                })
            {
                return Err(bad(
                    "extraction of editor tracking, COGO or attachments is not supported",
                ));
            }
        }
        let table = src.open_table(&name)?;
        if table.oid_field_name() != Some("OBJECTID") {
            return Err(bad("extraction requires a standard OBJECTID field"));
        }
        let rows: Vec<_> = table.features().collect::<Result<_>>()?;
        let is_explicit = explicit.contains(&name);
        let all = !options.schema_only
            && (if table.geometry_kind().is_none() {
                options.all_table_rows || (!is_explicit && options.expand == Expand::AllRows)
            } else {
                is_explicit || options.expand != Expand::SchemaOnly
            });
        let keep = if all {
            (0..rows.len()).collect()
        } else {
            BTreeSet::new()
        };
        selected.insert(
            name,
            Selected {
                table,
                rows,
                selected: keep,
            },
        );
    }
    if !options.schema_only && options.related && options.expand != Expand::None {
        // Follow both directions to a fixed point; NULL never joins to NULL.
        loop {
            let before: usize = selected.values().map(|s| s.selected.len()).sum();
            for r in &relationships {
                expand_related(&mut selected, r, false)?;
                expand_related(&mut selected, r, true)?;
            }
            if selected.values().map(|s| s.selected.len()).sum::<usize>() == before {
                break;
            }
        }
    }
    let existing = if create_new {
        None
    } else {
        Some(Geodatabase::open(destination)?)
    };
    let mut domains = BTreeSet::new();
    for (name, s) in &selected {
        if existing.as_ref().is_some_and(|d| {
            d.entries()
                .iter()
                .any(|e| e.name.eq_ignore_ascii_case(name))
                || d.relationship_classes()
                    .iter()
                    .any(|r| r.name.eq_ignore_ascii_case(name))
        }) {
            return Err(bad(format!(
                "ERROR 000258: Output {name} already exists; schema reuse is not supported"
            )));
        }
        for field in s.table.all_fields() {
            if let Some(domain) = &field.domain {
                domains.insert(domain.clone());
            }
        }
        if let Some(st) = s.table.subtypes() {
            domains.extend(
                st.subtypes
                    .iter()
                    .flat_map(|s| &s.fields)
                    .filter_map(|f| f.domain.clone()),
            );
        }
    }
    for name in &domains {
        let domain = src
            .domain(name)
            .ok_or_else(|| bad(format!("missing domain {name}")))?;
        if let Some(other) = existing.as_ref().and_then(|d| d.domain(name))
            && other != domain
        {
            return Err(bad(format!("incompatible destination domain {name}")));
        }
    }
    for r in &relationships {
        if existing.as_ref().is_some_and(|d| {
            d.entries()
                .iter()
                .any(|e| e.name.eq_ignore_ascii_case(&r.name))
                || d.relationship_classes()
                    .iter()
                    .any(|other| other.name.eq_ignore_ascii_case(&r.name))
        }) {
            return Err(bad(format!(
                "ERROR 000258: Output {} already exists",
                r.name
            )));
        }
    }
    let mut writer = if create_new {
        GeodatabaseWriter::create(destination)?
    } else {
        GeodatabaseWriter::open_existing(destination)?
    };
    let write = || -> Result<()> {
        for name in &domains {
            if !writer.has_domain(name) {
                writer.add_domain(src.domain(name).ok_or_else(|| bad("missing domain"))?)?;
            }
        }
        for (name, s) in &selected {
            // Copy the original storage schema, domain/default/alias and coordinate
            // resolution. Keep OID gaps even when only a subset of rows is extracted.
            let id = writer.next_table;
            writer.next_table += 1;
            let fields = s.table.all_fields().to_vec();
            let mut raw = TableWriter::create(
                &writer.table_path(id),
                fields.clone(),
                s.table.geometry_kind(),
                s.table.has_z(),
                s.table.has_m(),
            )?;
            for ix in s.table.indexes() {
                if s.table
                    .fields()
                    .iter()
                    .any(|f| f.name.eq_ignore_ascii_case(&ix.expression))
                {
                    raw.add_index(&ix.name, &ix.expression)?;
                } else if !s
                    .table
                    .oid_field_name()
                    .is_some_and(|f| f.eq_ignore_ascii_case(&ix.expression))
                    && !s
                        .table
                        .shape_field()
                        .is_some_and(|f| f.name.eq_ignore_ascii_case(&ix.expression))
                {
                    return Err(bad(format!(
                        "unsupported index expression {}",
                        ix.expression
                    )));
                }
            }
            for &i in &s.selected {
                let f = &s.rows[i];
                raw.push_at(f.oid as u64, f.geometry.as_ref(), &f.attributes)?;
            }
            let t = TableBuilder {
                standard_geometry_fields: false,
                subtypes: s.table.subtypes().cloned(),
                dataset: None,
                name: name.clone(),
                id,
                writer: raw,
                kind: s.table.geometry_kind(),
                has_z: s.table.has_z(),
                has_m: s.table.has_m(),
                def: s.table.geometry_def().cloned(),
                fields,
            };
            writer.finish_table(t)?;
        }
        for r in relationships {
            writer.add_relationship_class(r)?;
        }
        writer.finish()
    };
    let result = write();
    if result.is_err() && create_new && destination.exists() {
        std::fs::remove_dir_all(destination).map_err(|e| Error::io(destination, e))?;
    }
    result
}

fn expand_related(
    selected: &mut BTreeMap<String, Selected>,
    r: &RelationshipClass,
    reverse: bool,
) -> Result<()> {
    let (from, to, key, fk) = if reverse {
        (
            &r.destination,
            &r.origin,
            &r.origin_foreign_key,
            &r.origin_primary_key,
        )
    } else {
        (
            &r.origin,
            &r.destination,
            &r.origin_primary_key,
            &r.origin_foreign_key,
        )
    };
    let source = selected
        .get(from)
        .ok_or_else(|| bad("missing relationship source"))?;
    let source_i = source
        .table
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(key))
        .ok_or_else(|| bad("unsupported relationship key"))?;
    let values: Vec<Value> = source
        .selected
        .iter()
        .map(|&i| source.rows[i].attributes[source_i].clone())
        .filter(|v| *v != Value::Null)
        .collect();
    let target = selected
        .get_mut(to)
        .ok_or_else(|| bad("missing relationship target"))?;
    let target_i = target
        .table
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(fk))
        .ok_or_else(|| bad("unsupported relationship key"))?;
    for (i, row) in target.rows.iter().enumerate() {
        if values.iter().any(|v| {
            crate::compare_values(v, &row.attributes[target_i]) == Some(std::cmp::Ordering::Equal)
        }) {
            target.selected.insert(i);
        }
    }
    Ok(())
}
