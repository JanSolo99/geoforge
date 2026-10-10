//! A `.gdb` directory: the system catalog and item paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use geoforge_core::{Error, Result, Value};

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
    /// Lower-case table name → its high-precision date fields.
    high_precision: HashMap<String, Vec<String>>,
}

impl Geodatabase {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
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
        let mut high_precision = HashMap::new();
        if let Some((_, id)) = all.iter().find(|(n, _)| n == "GDB_Items") {
            if let Ok(items) = Table::open(table_path(&dir, *id)) {
                let def_i = field_index(&items, "Definition").ok();
                if let (Ok(ni), Ok(pi)) = (field_index(&items, "Name"), field_index(&items, "Path"))
                {
                    for row in items.features().flatten() {
                        if let (Value::String(n), Value::String(p)) =
                            (&row.attributes[ni], &row.attributes[pi])
                        {
                            paths.push((n.clone(), p.clone()));
                            if let Some(Value::Xml(xml)) = def_i.map(|i| &row.attributes[i]) {
                                let hp = high_precision_fields(xml);
                                if !hp.is_empty() {
                                    high_precision.insert(n.to_lowercase(), hp);
                                }
                            }
                        }
                    }
                }
            }
        }
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
            high_precision,
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

    /// Opens a table or feature class by name (case-insensitive, as in ArcGIS).
    pub fn open_table(&self, name: &str) -> Result<Table> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::io(self.dir.join(name), std::io::ErrorKind::NotFound.into()))?;
        let mut table = Table::open(table_path(&self.dir, entry.table_id))?;
        if let Some(hp) = self.high_precision.get(&entry.name.to_lowercase()) {
            table.set_high_precision(hp);
        }
        Ok(table)
    }
}

/// Names of `<GPFieldInfoEx>` fields marked `<HighPrecision>true</HighPrecision>`.
fn high_precision_fields(xml: &str) -> Vec<String> {
    xml.split("<GPFieldInfoEx")
        .skip(1)
        .filter_map(|chunk| {
            let chunk = chunk.split("</GPFieldInfoEx>").next()?;
            if !chunk.contains("<HighPrecision>true</HighPrecision>") {
                return None;
            }
            let name = chunk.split("<Name>").nth(1)?.split("</Name>").next()?;
            Some(name.to_owned())
        })
        .collect()
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
