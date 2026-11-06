//! A `.gdb` directory: the system catalog and item paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use unigis_core::{Error, Result, Value};

use crate::schema::{Domain, ItemDefinition, RelationshipClass, TableDefinition, parse_definition};
use crate::store::Store;
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

/// Any item of the geodatabase (`GDB_Items`): tables and feature classes, and the rest ArcGIS's
/// Catalog lists (relationship classes, topologies, network datasets, mosaic and raster datasets,
/// toolboxes, domains…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GdbItem {
    pub name: String,
    /// `\Dataset\Name`.
    pub path: String,
    /// The item type's name from `GDB_ItemTypes` (`Feature Class`, `Topology`, …), or its GUID
    /// when the geodatabase doesn't name it.
    pub kind: String,
    /// A feature class's feature type from its definition (`esriFTSimple`, `esriFTAnnotation`,
    /// `esriFTDimension`, …).
    pub feature_type: Option<String>,
    /// The feature dataset holding it.
    pub feature_dataset: Option<String>,
    /// A raster dataset's coordinate system from its definition (`<SpatialReference><WKT>`),
    /// for one whose bands don't name a `GDB_SpatialRefs` row.
    pub srs_wkt: Option<String>,
}

/// A `GDB_Items` row as read: name, path, type GUID, feature type, a raster's WKT.
type RawItem = (String, String, String, Option<String>, Option<String>);

/// The `GDB_Items` type of raster datasets.
const RASTER_DATASET_TYPE: &str = "{5ED667A3-9CA9-44A2-8029-D95BF23704B9}";

pub struct Geodatabase {
    dir: PathBuf,
    store: Store,
    items: Vec<GdbItem>,
    /// Every table in the system catalog (system tables too), by name and number.
    catalog: Vec<(String, u64)>,
    entries: Vec<CatalogEntry>,
    feature_datasets: Vec<String>,
    /// Lower-case table name → schema from its item definition.
    definitions: HashMap<String, TableDefinition>,
    domains: Vec<Domain>,
    relationships: Vec<RelationshipClass>,
}

/// The `GDB_Items` type of feature datasets.
const FEATURE_DATASET_TYPE: &str = "{74737149-DCB5-4257-8904-B9724E32A530}";

impl Geodatabase {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        // An edit that was interrupted is rolled back first (see `writer::recover`).
        crate::writer::recover(&dir)?;
        Self::open_in(Store::disk(), dir)
    }

    /// Reads a geodatabase from files held in memory (a browser upload or a zip archive):
    /// `name` is the `.gdb` folder's name, `files` its files by name. Folders around them don't
    /// matter, and files that aren't part of a geodatabase are ignored.
    pub fn from_files(
        name: impl AsRef<Path>,
        files: impl IntoIterator<Item = (String, Vec<u8>)>,
    ) -> Result<Self> {
        Self::open_in(Store::memory(files), name.as_ref().to_path_buf())
    }

    /// Reads the geodatabase in `dir` from `store`. Unlike [`Geodatabase::open`], it never
    /// rolls back an interrupted edit, so it only reads.
    pub fn open_in(store: Store, dir: PathBuf) -> Result<Self> {
        let catalog = Table::open_in(&store, table_path(&dir, 1))?;
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
        let mut typed_datasets = Vec::new();
        // (name, path, type GUID, feature type, a raster's WKT)
        let mut raw_items: Vec<RawItem> = Vec::new();
        if let Some((_, id)) = all.iter().find(|(n, _)| n == "GDB_Items")
            && let Ok(items) = Table::open_in(&store, table_path(&dir, *id))
        {
            let def_i = field_index(&items, "Definition").ok();
            let type_i = field_index(&items, "Type").ok();
            if let (Ok(ni), Ok(pi)) = (field_index(&items, "Name"), field_index(&items, "Path")) {
                for row in items.features().flatten() {
                    let (Value::String(n), Value::String(p)) =
                        (&row.attributes[ni], &row.attributes[pi])
                    else {
                        continue;
                    };
                    // A feature dataset, empty or not, by its item type.
                    if let Some(Value::String(t) | Value::Guid(t)) =
                        type_i.map(|i| &row.attributes[i])
                        && t.eq_ignore_ascii_case(FEATURE_DATASET_TYPE)
                    {
                        typed_datasets.push(n.clone());
                    }
                    paths.push((n.clone(), p.clone()));
                    let guid = match type_i.map(|i| &row.attributes[i]) {
                        Some(Value::String(t) | Value::Guid(t)) => t.clone(),
                        _ => String::new(),
                    };
                    let xml = match def_i.map(|i| &row.attributes[i]) {
                        Some(Value::Xml(xml)) => Some(xml),
                        _ => None,
                    };
                    let feature_type = xml.and_then(|x| tag_text(x, "FeatureType"));
                    let srs = xml
                        .filter(|_| guid.eq_ignore_ascii_case(RASTER_DATASET_TYPE))
                        .and_then(|x| tag_text(x, "WKT"));
                    raw_items.push((n.clone(), p.clone(), guid, feature_type, srs));
                    let Some(xml) = xml else {
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
        // Item type names, as the geodatabase itself lists them.
        let mut type_names: HashMap<String, String> = HashMap::new();
        if let Some((_, id)) = all.iter().find(|(n, _)| n == "GDB_ItemTypes")
            && let Ok(types) = Table::open_in(&store, table_path(&dir, *id))
            && let (Ok(ui), Ok(ni)) = (field_index(&types, "UUID"), field_index(&types, "Name"))
        {
            for row in types.features().flatten() {
                if let (Value::String(u) | Value::Guid(u), Value::String(n)) =
                    (&row.attributes[ui], &row.attributes[ni])
                {
                    type_names.insert(u.to_ascii_uppercase(), n.clone());
                }
            }
        }
        let items: Vec<GdbItem> = raw_items
            .into_iter()
            .map(|(name, path, guid, feature_type, srs_wkt)| {
                let mut parts = path.trim_start_matches('\\').split('\\');
                let first = parts.next().map(str::to_owned);
                let feature_dataset = parts.next().and(first);
                GdbItem {
                    kind: type_names
                        .get(&guid.to_ascii_uppercase())
                        .cloned()
                        .unwrap_or(guid),
                    name,
                    path,
                    feature_type,
                    feature_dataset,
                    srs_wkt,
                }
            })
            .collect();
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
            .filter(|(name, id)| !name.starts_with("GDB_") && store.is_file(&table_path(&dir, *id)))
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
        feature_datasets.extend(typed_datasets);
        feature_datasets.sort_unstable();
        feature_datasets.dedup();

        Ok(Self {
            dir,
            store,
            items,
            catalog: all,
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

    /// Every table of the system catalog (`GDB_*` system tables too): name and `aXXXXXXXX` number.
    pub fn catalog(&self) -> &[(String, u64)] {
        &self.catalog
    }

    /// Every item `GDB_Items` lists, in its order, with its type's name.
    pub fn items(&self) -> &[GdbItem] {
        &self.items
    }

    /// Where its files are read from.
    pub fn store(&self) -> &Store {
        &self.store
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
        let mut table = Table::open_in(&self.store, table_path(&self.dir, entry.table_id))?;
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

/// The text of the first `<tag>` element in an item definition.
fn tag_text(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = start + xml.get(start..)?.find("</")?;
    let text = xml.get(start..end)?.trim();
    // XML's predefined entities (a WKT's quotes may be written `&quot;`).
    let text = text
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    Some(text).filter(|t| !t.is_empty())
}
