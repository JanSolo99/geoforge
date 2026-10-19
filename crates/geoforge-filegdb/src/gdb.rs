//! A `.gdb` directory: the system catalog and item paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use geoforge_core::{Error, Result, Value};

use crate::schema::{Domain, ItemDefinition, RelationshipClass, TableDefinition, parse_definition};
use crate::table::Table;

/// One user table or feature class listed in the system catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub name: String,
    /// The `aXXXXXXXX` number (the catalog OBJECTID).
    pub table_id: u64,
    /// Feature dataset containing this item, from `GDB_Items` (FileGDB 10+).
    pub feature_dataset: Option<String>,
}

pub struct Geodatabase {
    dir: PathBuf,
    entries: Vec<CatalogEntry>,
    feature_datasets: Vec<String>,
    /// Lower-case table name → schema from its item definition.
    definitions: HashMap<String, TableDefinition>,
    domains: Vec<Domain>,
    relationships: Vec<RelationshipClass>,
}

impl Geodatabase {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        // An edit that was interrupted is rolled back first (see `writer::recover`).
        crate::writer::recover(&dir)?;
        let catalog = Table::open(table_path(&dir, 1))?;
        let name_i = field_index(&catalog, "Name")?;
        let mut all = Vec::new();
        for row in catalog.features() {
            let row = row?;
            if let Value::String(name) = &row.attributes[name_i] {
                all.push((name.clone(), row.oid as u64));
            }
        }

        // Item paths (`\Dataset\Name`) live in GDB_Items, located through the catalog.
        let mut paths = Vec::new();
        let mut definitions = HashMap::new();
        let mut domains = Vec::new();
        let mut relationships = Vec::new();
        if let Some((_, id)) = all.iter().find(|(n, _)| n == "GDB_Items")
            && let Ok(items) = Table::open(table_path(&dir, *id))
        {
            let def_i = field_index(&items, "Definition").ok();
            if let (Ok(ni), Ok(pi)) = (field_index(&items, "Name"), field_index(&items, "Path")) {
                for row in items.features().flatten() {
                    let (Value::String(n), Value::String(p)) =
                        (&row.attributes[ni], &row.attributes[pi])
                    else {
                        continue;
                    };
                    paths.push((n.clone(), p.clone()));
                    let Some(Value::Xml(xml)) = def_i.map(|i| &row.attributes[i]) else {
                        continue;
                    };
                    match parse_definition(xml) {
                        ItemDefinition::Domain(d) => domains.push(d),
                        ItemDefinition::Relationship(r) => relationships.push(r),
                        ItemDefinition::Table(t) => {
                            definitions.insert(n.to_lowercase(), t);
                        }
                        ItemDefinition::Other => {}
                    }
                }
            }
        }
        domains.sort_by(|a, b| a.name.cmp(&b.name));
        relationships.sort_by(|a, b| a.name.cmp(&b.name));
        let dataset_of = |name: &str| {
            let (_, path) = paths.iter().find(|(n, _)| n.eq_ignore_ascii_case(name))?;
            let mut parts = path.trim_start_matches('\\').split('\\');
            let first = parts.next()?;
            parts.next().map(|_| first.to_owned())
        };

        let entries: Vec<CatalogEntry> = all
            .iter()
            .filter(|(name, id)| !name.starts_with("GDB_") && table_path(&dir, *id).is_file())
            .map(|(name, id)| CatalogEntry {
                name: name.clone(),
                table_id: *id,
                feature_dataset: dataset_of(name),
            })
            .collect();

        // Feature datasets are items whose path has one level and that are not tables.
        let mut feature_datasets: Vec<String> = entries
            .iter()
            .filter_map(|e| e.feature_dataset.clone())
            .collect();
        for (name, path) in &paths {
            let is_table = all.iter().any(|(n, _)| n.eq_ignore_ascii_case(name));
            let one_level =
                path.trim_start_matches('\\').split('\\').count() == 1 && !path.is_empty();
            if one_level && !is_table && name != "Workspace" && !name.is_empty() {
                // Only keep it if something lives under it (other one-level items are domains etc.).
                if entries
                    .iter()
                    .any(|e| e.feature_dataset.as_deref() == Some(name.as_str()))
                {
                    feature_datasets.push(name.clone());
                }
            }
        }
        feature_datasets.sort_unstable();
        feature_datasets.dedup();

        Ok(Self {
            dir,
            entries,
            feature_datasets,
            definitions,
            domains,
            relationships,
        })
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// User tables and feature classes, in catalog order.
    pub fn entries(&self) -> &[CatalogEntry] {
        &self.entries
    }

    pub fn feature_datasets(&self) -> &[String] {
        &self.feature_datasets
    }

    /// Attribute domains, sorted by name (`arcpy.da.ListDomains`).
    pub fn domains(&self) -> &[Domain] {
        &self.domains
    }

    pub fn domain(&self, name: &str) -> Option<&Domain> {
        self.domains
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
    }

    /// Relationship classes, sorted by name.
    pub fn relationship_classes(&self) -> &[RelationshipClass] {
        &self.relationships
    }

    /// Opens a table or feature class by name (case-insensitive, as in ArcGIS).
    /// The path of a table's files without extension (`<gdb>/aXXXXXXXX`).
    pub fn table_base(&self, name: &str) -> Result<PathBuf> {
        let id = self
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .map(|e| e.table_id)
            .ok_or_else(|| {
                Error::Unsupported(format!("no table {name} in {}", self.dir.display()))
            })?;
        Ok(table_path(&self.dir, id))
    }

    pub fn open_table(&self, name: &str) -> Result<Table> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::io(self.dir.join(name), std::io::ErrorKind::NotFound.into()))?;
        let mut table = Table::open(table_path(&self.dir, entry.table_id))?;
        if let Some(def) = self.definitions.get(&entry.name.to_lowercase()) {
            table.apply_definition(def);
        }
        Ok(table)
    }
}

fn table_path(dir: &Path, id: u64) -> PathBuf {
    dir.join(format!("a{id:08x}.gdbtable"))
}

fn field_index(table: &Table, name: &str) -> Result<usize> {
    table
        .fields()
        .iter()
        .position(|f| f.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| Error::format("FileGDB", format!("system table lacks field {name}")))
}
