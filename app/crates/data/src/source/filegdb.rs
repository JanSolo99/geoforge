//! File Geodatabases (`.gdb` folders) read in place through UniGIS's `unigis-filegdb`.
//!
//! Modified for UniGIS (not part of upstream RustGIS).
//!
//! `open` picks a feature class or table by name (default: the first feature class); a read asks
//! the table's spatial index (`.spx`) for the rows near the extent, then reads just those rows.
//! Curves are densified, as the map draws straight segments. MultiPatch classes are not drawn.
//!
//! The files are hostile input: a row that doesn't decode is skipped and counted.

use std::path::Path;
use std::sync::{Arc, Mutex, Weak};

use unigis_core::{Envelope, GeometryKind, Value as GfValue};
use unigis_filegdb::Table;
pub use unigis_filegdb::{Geodatabase, vfs};

pub use super::gfshape::{from_unigis, to_unigis};
use super::{Batch, FeatureSource, Reproject, Request, SourceInfo, clamp_limit, stride};
use crate::{DataError, Dataset, Feature, Field, FieldType, Geometry, GeometryType, Result, Value};
use rustgis_geom::Extent;

fn bad(msg: impl std::fmt::Display) -> DataError {
    DataError::Format(format!("File Geodatabase: {msg}"))
}

/// A feature class or table of a File Geodatabase, read in place.
pub struct FileGdbSource {
    info: SourceInfo,
    table: Table,
    reproject: Reproject,
}

impl FileGdbSource {
    /// Opens `path` (the `.gdb` folder); `layer` names a table (default: the first feature class,
    /// else the first table).
    pub fn open(path: &Path, layer: Option<&str>) -> Result<FileGdbSource> {
        Self::open_in(&Geodatabase::open(path).map_err(bad)?, layer)
    }

    /// Opens a geodatabase uploaded as a zip archive of its folder's files (see [`zip_geodatabases`]).
    pub fn open_zip(bytes: &super::Shared, layer: Option<&str>) -> Result<FileGdbSource> {
        Self::open_in(&*from_zip(bytes)?, layer)
    }

    /// Opens a table of an open geodatabase.
    pub fn open_in(gdb: &Geodatabase, layer: Option<&str>) -> Result<FileGdbSource> {
        let name = match layer {
            Some(n) => n.to_string(),
            None => {
                let mut first_table = None;
                let mut found = None;
                for e in gdb.entries() {
                    let t = gdb.open_table(&e.name).map_err(bad)?;
                    if t.geometry_kind().is_some() {
                        found = Some(e.name.clone());
                        break;
                    }
                    first_table.get_or_insert_with(|| e.name.clone());
                }
                found.or(first_table).ok_or_else(|| bad("the geodatabase holds no tables"))?
            }
        };
        let table = gdb.open_table(&name).map_err(bad)?;
        let name = gdb.entries().iter().find(|e| e.name.eq_ignore_ascii_case(&name)).map_or(name, |e| e.name.clone());
        let mut notes = Vec::new();
        let geometry_type = match table.geometry_kind() {
            None => GeometryType::None,
            Some(GeometryKind::Point | GeometryKind::Multipoint) => GeometryType::Point,
            Some(GeometryKind::Polyline) => GeometryType::Polyline,
            Some(GeometryKind::Polygon) => GeometryType::Polygon,
            // Drawn by its footprint, as ArcGIS draws a MultiPatch on a 2D map.
            Some(GeometryKind::MultiPatch) => {
                notes.push("a MultiPatch class: drawn by its footprint on the 2D map".into());
                GeometryType::Polygon
            }
        };
        let def = table.geometry_def();
        let crs = match def.and_then(|d| d.srs_wkt.as_deref()) {
            None => crate::raster::Crs::LonLat,
            Some(wkt) => match crate::raster::wkt::read_wkt(wkt) {
                Ok((c, note)) => {
                    notes.extend(note.map(|n| format!("the coordinate system: {n}")));
                    c
                }
                Err(e) => {
                    notes.push(format!("the coordinate system: {e}; coordinates are read as longitude/latitude"));
                    crate::raster::Crs::LonLat
                }
            },
        };
        let reproject = Reproject::from_crs(crs, &mut notes);
        let extent = def
            .map(|d| Extent::new(d.extent.xmin, d.extent.ymin, d.extent.xmax, d.extent.ymax))
            .filter(|e| e.min_x.is_finite() && e.max_x.is_finite() && e.min_x <= e.max_x && e.min_y <= e.max_y)
            .map_or(Extent::EMPTY, |e| reproject.extent_to_lonlat(&e));
        let fields = table.fields().iter().map(|f| Field::new(&f.name, field_type(f.field_type))).collect();
        let indexed = def.is_some_and(|d| d.grid_sizes.iter().any(|&g| g > 0.0)) && has_spx(gdb, &name);
        let info = SourceInfo {
            name,
            format: "File Geodatabase",
            geometry_type,
            fields,
            extent,
            count: Some(table.row_count()),
            crs: reproject.label(),
            indexed,
            notes,
        };
        Ok(FileGdbSource { info, table, reproject })
    }

    /// The OBJECTIDs a request has to look at, and whether they came from the spatial index (else
    /// every row).
    fn candidates(&self, env: Option<&Envelope>) -> (Vec<u64>, bool) {
        if let Some(env) = env
            && let Ok(Some(oids)) = self.table.spatial_index_candidates(env)
        {
            return (oids, true);
        }
        ((1..=self.table.max_oid()).collect(), false)
    }
}

fn has_spx(gdb: &Geodatabase, name: &str) -> bool {
    gdb.table_base(name).is_ok_and(|b| gdb.store().is_file(&b.with_extension("spx")))
}

/// Whether a file name is a geodatabase's system catalog (`a00000001.gdbtable`), which marks
/// the folder holding it as a geodatabase.
fn is_catalog(name: &str) -> bool {
    name.rsplit(['/', '\\']).next().is_some_and(|n| n.eq_ignore_ascii_case("a00000001.gdbtable"))
}

/// The folder part of a zip entry's name (`""` for one at the top).
fn folder(name: &str) -> &str {
    name.rfind(['/', '\\']).map_or("", |i| &name[..i])
}

/// Largest geodatabase read from memory (an upload): 1 GiB in the browser, 4 GiB (a zip
/// archive's limit) on the desktop. It is held once (an uncompressed archive is read in place).
pub const MAX_GEODATABASE_BYTES: usize = if cfg!(target_arch = "wasm32") { 1 << 30 } else { u32::MAX as usize };

/// The geodatabases in a zip archive, each as `(Name.gdb, a zip archive of just its files)`: a
/// folder is one when it holds a system catalog (`a00000001.gdbtable`). Files zipped without
/// their folder make one named after the archive (`file_name`). Other files are left out. An
/// uncompressed archive holding just one geodatabase (a picked folder) is used as it is, not
/// copied.
pub fn zip_geodatabases(file_name: &str, bytes: Arc<Vec<u8>>) -> Result<Vec<(String, Arc<Vec<u8>>)>> {
    let names = crate::zip::read_matching(&bytes, is_catalog).map_err(|e| bad(format!("{file_name}: {e}")))?;
    let mut folders: Vec<String> = names.iter().map(|(n, _)| folder(n).to_string()).collect();
    folders.sort();
    folders.dedup();
    if folders.is_empty() {
        return Ok(Vec::new());
    }
    let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("Upload");
    let gdb_name = |f: &str| {
        let leaf = f.rsplit(['/', '\\']).next().unwrap_or(f);
        let base = if leaf.is_empty() { stem } else { leaf };
        let base = base.strip_suffix(".gdb").or_else(|| base.strip_suffix(".GDB")).unwrap_or(base);
        format!("{base}.gdb")
    };
    // One geodatabase and nothing else, stored: the archive is already what's needed.
    if let [only] = folders.as_slice()
        && let Ok(Some(all)) = crate::zip::stored_ranges(&bytes, |_| true)
        && all.iter().all(|(n, _)| folder(n) == only)
    {
        return Ok(vec![(gdb_name(only), bytes)]);
    }
    let wanted = |n: &str| folders.iter().any(|f| folder(n) == f) && !n.ends_with('/');
    let entries = crate::zip::read_matching_within(&bytes, wanted, MAX_GEODATABASE_BYTES).map_err(|e| bad(format!("{file_name}: {e}")))?;
    let mut out = Vec::new();
    for f in &folders {
        let mut w = crate::zip::StoredWriter::default();
        for (n, b) in entries.iter().filter(|(n, _)| folder(n) == f) {
            w.add(n.rsplit(['/', '\\']).next().unwrap_or(n), b).map_err(bad)?;
        }
        out.push((gdb_name(f), Arc::new(w.finish().map_err(bad)?)));
    }
    Ok(out)
}

/// Whether a zip archive holds a geodatabase (reads only its directory and the catalog).
pub fn zip_has_geodatabase(bytes: &[u8]) -> bool {
    crate::zip::read_matching(bytes, is_catalog).is_ok_and(|e| !e.is_empty())
}

/// Geodatabases recently read from zip archives, by the archive's bytes: a layer per feature
/// class reopens its geodatabase, which needn't be inflated again each time.
static OPENED: Mutex<Vec<(Weak<Vec<u8>>, Arc<Geodatabase>)>> = Mutex::new(Vec::new());

/// The files of a geodatabase held as a zip archive of its folder (an upload), by name.
pub fn zip_files(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    let files = crate::zip::read_matching_within(bytes, |n| !n.ends_with('/'), MAX_GEODATABASE_BYTES).map_err(bad)?;
    Ok(files.into_iter().map(|(n, b)| (n.rsplit(['/', '\\']).next().unwrap_or(&n).to_string(), b)).collect())
}

/// A geodatabase's files as an uncompressed zip archive (how an upload is held).
pub fn to_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut w = crate::zip::StoredWriter::default();
    for (n, b) in files {
        w.add(n, b).map_err(bad)?;
    }
    w.finish().map_err(bad)
}

/// Deletes an item (and what it holds), returning what was removed.
pub fn vfs_delete_item(dir: &Path, item: &str) -> Result<Vec<String>> {
    unigis_filegdb::delete_item(dir, item).map_err(bad)
}

/// Renames an item, as ArcGIS's Rename does.
pub fn vfs_rename_item(dir: &Path, item: &str, new: &str) -> Result<()> {
    unigis_filegdb::rename_item(dir, item, new).map_err(bad)
}

/// Creates an empty file geodatabase at `dir`, as ArcGIS's Create File Geodatabase does (on
/// disk, or under a folder mounted in memory).
pub fn create_empty(dir: &Path) -> Result<()> {
    unigis_filegdb::GeodatabaseWriter::create(dir).and_then(|w| w.finish()).map_err(bad)
}

/// The files directly in a folder (through `vfs`, so a memory folder too), by name.
pub fn dir_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for e in vfs::read_dir(dir).map_err(|e| bad(e.to_string()))? {
        let p = e.map_err(|e| bad(e.to_string()))?.path();
        if vfs::is_file(&p) {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
            out.push((name, vfs::read(&p).map_err(|e| bad(e.to_string()))?));
        }
    }
    Ok(out)
}

/// Opens the geodatabase in a `.gdb` folder on disk.
pub fn open_dir(path: &Path) -> Result<Arc<Geodatabase>> {
    Geodatabase::open(path).map(Arc::new).map_err(bad)
}

/// Reads a geodatabase held as a zip archive of its folder's files.
pub fn from_zip(bytes: &super::Shared) -> Result<Arc<Geodatabase>> {
    let mut cache = OPENED.lock().unwrap_or_else(|e| e.into_inner());
    cache.retain(|(w, _)| w.strong_count() > 0);
    if let Some((_, g)) = cache.iter().find(|(w, _)| w.upgrade().is_some_and(|b| Arc::ptr_eq(&b, &bytes.0))) {
        return Ok(g.clone());
    }
    // Stored entries are read in place; compressed ones are inflated once.
    let store = match crate::zip::stored_ranges(bytes.as_ref(), |_| true).map_err(bad)? {
        Some(ranges) => unigis_filegdb::Store::shared(bytes.0.clone(), ranges),
        None => {
            let files = crate::zip::read_matching_within(bytes.as_ref(), |_| true, MAX_GEODATABASE_BYTES).map_err(bad)?;
            unigis_filegdb::Store::memory(files)
        }
    };
    let gdb = Arc::new(Geodatabase::open_in(store, "upload.gdb".into()).map_err(bad)?);
    cache.push((Arc::downgrade(&bytes.0), gdb.clone()));
    // A handful at a time: each holds its geodatabase's files.
    if cache.len() > 8 {
        cache.remove(0);
    }
    Ok(gdb)
}

/// What a geodatabase holds, for a catalog: its feature datasets, its tables and feature classes
/// with the feature dataset each is in, every other item ArcGIS's Catalog lists (relationship
/// classes, topologies, network datasets, mosaic and raster datasets, toolboxes…), and its domains.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Contents {
    pub feature_datasets: Vec<String>,
    pub items: Vec<Item>,
    pub others: Vec<Other>,
    /// Attribute domains, by name.
    pub domains: Vec<String>,
}

/// One table or feature class of a geodatabase.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub feature_dataset: Option<String>,
    /// `None` for a table.
    pub geometry: Option<GeometryType>,
    /// A MultiPatch class (drawn by its footprint).
    pub multipatch: bool,
    /// A feature class's feature type when it isn't a simple one (`esriFTAnnotation`,
    /// `esriFTDimension`, …).
    pub feature_type: Option<String>,
    pub rows: u64,
}

/// An item that isn't a table or feature class: its name and type (`Relationship Class`,
/// `Topology`, `Mosaic Dataset`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Other {
    pub name: String,
    pub kind: String,
    pub feature_dataset: Option<String>,
}

/// Item types the Catalog doesn't list as items of their own (the geodatabase itself, its feature
/// datasets, tables and classes, domains, extensions).
const NOT_LISTED: &[&str] = &[
    "Workspace",
    "Workspace Extension",
    "Feature Dataset",
    "Feature Class",
    "Table",
    "Coded Value Domain",
    "Range Domain",
    "Folder",
    "Resource",
];

/// The contents of an open geodatabase. Tables that don't open are listed as tables.
pub fn contents(gdb: &Geodatabase) -> Contents {
    let kind_of = |name: &str| gdb.items().iter().find(|i| i.name.eq_ignore_ascii_case(name));
    // A mosaic dataset's own tables (`AMD_<name>_…`) are part of it, as ArcGIS shows it.
    let mosaics: Vec<String> =
        gdb.items().iter().filter(|i| i.kind == "Mosaic Dataset").map(|i| format!("AMD_{}_", i.name).to_ascii_lowercase()).collect();
    let in_mosaic = |name: &str| mosaics.iter().any(|m| name.to_ascii_lowercase().starts_with(m.as_str()));
    let mut items = Vec::new();
    for e in gdb.entries() {
        let item = kind_of(&e.name);
        // Tables that back other items (an attributed relationship class, a raster catalog) are
        // listed as those items.
        if item.is_some_and(|i| !["Feature Class", "Table"].contains(&i.kind.as_str())) || in_mosaic(&e.name) {
            continue;
        }
        let t = gdb.open_table(&e.name).ok();
        let kind = t.as_ref().and_then(Table::geometry_kind);
        items.push(Item {
            name: e.name.clone(),
            feature_dataset: e.feature_dataset.clone(),
            geometry: kind.map(|k| match k {
                GeometryKind::Point | GeometryKind::Multipoint => GeometryType::Point,
                GeometryKind::Polyline => GeometryType::Polyline,
                GeometryKind::Polygon | GeometryKind::MultiPatch => GeometryType::Polygon,
            }),
            multipatch: kind == Some(GeometryKind::MultiPatch),
            feature_type: item.and_then(|i| i.feature_type.clone()).filter(|f| f != "esriFTSimple"),
            rows: t.as_ref().map_or(0, Table::row_count),
        });
    }
    let others = gdb
        .items()
        .iter()
        .filter(|i| !NOT_LISTED.contains(&i.kind.as_str()) && !i.name.is_empty() && !in_mosaic(&i.name) && !i.kind.starts_with('{'))
        .map(|i| Other { name: i.name.clone(), kind: i.kind.clone(), feature_dataset: i.feature_dataset.clone() })
        .collect();
    Contents {
        feature_datasets: gdb.feature_datasets().to_vec(),
        items,
        others,
        domains: gdb.domains().iter().map(|d| d.name.clone()).collect(),
    }
}

fn field_type(t: unigis_core::FieldType) -> FieldType {
    use unigis_core::FieldType as G;
    match t {
        G::SmallInteger | G::Integer | G::BigInteger => FieldType::Integer,
        G::Single | G::Double => FieldType::Double,
        _ => FieldType::Text,
    }
}

fn value(v: GfValue) -> Value {
    match v {
        GfValue::Null => Value::Null,
        GfValue::SmallInteger(i) => Value::Int(i64::from(i)),
        GfValue::Integer(i) => Value::Int(i64::from(i)),
        GfValue::BigInteger(i) => Value::Int(i),
        GfValue::Single(f) => Value::Float(f64::from(f)),
        GfValue::Double(f) => Value::Float(f),
        GfValue::String(s) | GfValue::Guid(s) | GfValue::Xml(s) => Value::Text(s),
        GfValue::Date(d) => Value::Text(d.to_string()),
        GfValue::DateOnly(d) => Value::Text(d.to_string()),
        GfValue::TimeOnly(t) => Value::Text(t.to_string()),
        GfValue::TimestampOffset(t) => Value::Text(t.to_rfc3339()),
        GfValue::Blob(b) => Value::Text(format!("<{} bytes>", b.len())),
    }
}

impl FeatureSource for FileGdbSource {
    fn info(&self) -> &SourceInfo {
        &self.info
    }

    fn read(&self, req: &Request) -> Result<Batch> {
        let limit = clamp_limit(req.limit);
        let native = req.extent.map(|e| self.reproject.extent_from_lonlat(&e));
        // An empty native extent (outside the projection's domain) matches nothing.
        if native.is_some_and(|e| e.is_empty()) {
            return Ok(Batch { complete: true, ..Batch::default() });
        }
        let env = native.map(|e| Envelope { xmin: e.min_x, ymin: e.min_y, xmax: e.max_x, ymax: e.max_y, z: None, m: None });
        let (mut oids, from_index) = self.candidates(env.as_ref());
        let mut skipped = 0;
        // Without a spatial index, or with an extent, the row's own envelope decides.
        let meets = |f: &unigis_core::Feature| match (&env, &f.geometry) {
            (None, _) => true,
            (Some(e), Some(g)) => {
                let b = g.envelope();
                !b.is_empty() && b.xmin <= e.xmax && b.xmax >= e.xmin && b.ymin <= e.ymax && b.ymax >= e.ymin
            }
            (Some(_), None) => false,
        };
        let mut rows: Vec<unigis_core::Feature> = Vec::new();
        let matched;
        let complete;
        if env.is_none() || from_index {
            // Candidates known up front: sample them, then read.
            oids.sort_unstable();
            oids.dedup();
            let step = stride(oids.len() as u64, limit).max(1) as usize;
            for oid in oids.iter().copied().step_by(step).take(limit) {
                match self.table.get(oid) {
                    Ok(Some(f)) if meets(&f) => rows.push(f),
                    Ok(_) => {}
                    Err(_) => skipped += 1,
                }
            }
            matched = oids.len() as u64;
            complete = step == 1;
        } else {
            // No usable index: scan, keep what meets the extent, then sample.
            for f in self.table.features() {
                match f {
                    Ok(f) if meets(&f) => rows.push(f),
                    Ok(_) => {}
                    Err(_) => skipped += 1,
                }
            }
            matched = rows.len() as u64;
            let step = stride(matched, limit).max(1) as usize;
            complete = step == 1;
            rows = rows.into_iter().step_by(step).take(limit).collect();
        }
        let has_geometry = self.info.geometry_type != GeometryType::None;
        let mut features = Vec::with_capacity(rows.len());
        for f in rows {
            let g = match (&f.geometry, has_geometry) {
                (Some(g), true) => match from_unigis(g).and_then(|g| self.reproject.geometry(g)) {
                    Some(g) => Some(g),
                    None => {
                        skipped += 1;
                        continue;
                    }
                },
                _ => None,
            };
            features.push(Feature { id: f.oid as u64, geometry: g, attrs: f.attributes.into_iter().map(value).collect() });
        }
        Ok(Batch { features, matched, complete, skipped })
    }
}

/// The sanitised name of a table or field for the geodatabase: letters, digits and `_`, starting
/// with a letter.
fn gdb_name(raw: &str, max: usize) -> String {
    let mut n: String = raw.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if !n.chars().next().is_some_and(char::is_alphabetic) {
        n.insert(0, 'F');
    }
    n.chars().take(max).collect()
}

/// The feature classes of a File Geodatabase, by name.
pub fn layers(gdb: &Path) -> Result<Vec<String>> {
    layers_in(&Geodatabase::open(gdb).map_err(bad)?)
}

/// The feature classes of a geodatabase uploaded as a zip archive, by name.
pub fn layers_zip(bytes: &super::Shared) -> Result<Vec<String>> {
    layers_in(&*from_zip(bytes)?)
}

/// The feature classes of an open geodatabase, by name.
pub fn layers_in(g: &Geodatabase) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for e in g.entries() {
        let t = g.open_table(&e.name).map_err(bad)?;
        if t.geometry_kind().is_some() {
            out.push(e.name.clone());
        }
    }
    out.sort_by_key(|n| n.to_ascii_lowercase());
    Ok(out)
}

/// Writes `ds` as a feature class (or table) named `name` (default: the layer's name) in the
/// File Geodatabase `gdb`, creating the geodatabase if it doesn't exist and replacing a class of
/// that name. Coordinates are WGS 84, as the map holds them. Returns the number of features.
pub fn write(ds: &Dataset, gdb: &Path, name: Option<&str>) -> Result<usize> {
    use unigis_core::{Feature as GfFeature, Field as GfField, FieldType as GfType, Value as GfValue};
    use unigis_filegdb::GeodatabaseWriter;
    let mut table = gdb_name(name.unwrap_or(&ds.name), 160);
    if table.to_ascii_uppercase().starts_with("GDB_") {
        table.insert(0, 'T');
    }
    // Through vfs: a geodatabase mounted in memory (an upload being edited) too.
    let exists = vfs::is_file(gdb.join("a00000001.gdbtable"));
    if vfs::exists(gdb) && !exists && vfs::read_dir(gdb).map_or(true, |mut d| d.next().is_some()) {
        return Err(bad(format!("{} exists and is not a geodatabase", gdb.display())));
    }
    if exists {
        // Replacing a class of the same name, as every other writer replaces its file.
        if Geodatabase::open(gdb).map_err(bad)?.entries().iter().any(|e| e.name.eq_ignore_ascii_case(&table)) {
            unigis_filegdb::delete_item(gdb, &table).map_err(bad)?;
        }
    }
    // Fields: names the geodatabase accepts, unique, and a type that holds every value.
    let mut fields: Vec<GfField> = Vec::new();
    let mut types: Vec<GfType> = Vec::new();
    for (i, f) in ds.fields.iter().enumerate() {
        let mut n = gdb_name(&f.name, 64);
        if ["OBJECTID", "SHAPE"].iter().any(|r| n.eq_ignore_ascii_case(r)) {
            n.push('_');
        }
        let base = n.clone();
        let mut k = 1;
        while fields.iter().any(|g| g.name.eq_ignore_ascii_case(&n)) {
            let suffix = format!("_{k}");
            n = format!("{}{suffix}", base.chars().take(64 - suffix.len()).collect::<String>());
            k += 1;
        }
        let column = || ds.features.iter().filter_map(move |ft| ft.attrs.get(i));
        let ty = match f.ty {
            FieldType::Integer if column().all(|v| !matches!(v, Value::Int(x) if i32::try_from(*x).is_err())) => GfType::Integer,
            FieldType::Integer | FieldType::Double => GfType::Double,
            FieldType::Boolean => GfType::SmallInteger,
            FieldType::Text => GfType::String,
        };
        let mut g = GfField::new(n, ty);
        g.alias = f.alias.clone().or_else(|| (g.name != f.name).then(|| f.name.clone()));
        if ty == GfType::String {
            let longest = column().map(|v| if let Value::Text(s) = v { s.chars().count() } else { 0 }).max().unwrap_or(0);
            g.length = longest.max(255) as u32;
        }
        types.push(ty);
        fields.push(g);
    }
    let kind = match ds.geometry_type {
        GeometryType::None => None,
        GeometryType::Point if ds.features.iter().any(|f| matches!(f.geometry, Some(Geometry::MultiPoint(_)))) => {
            Some(GeometryKind::Multipoint)
        }
        GeometryType::Point => Some(GeometryKind::Point),
        GeometryType::Polyline => Some(GeometryKind::Polyline),
        GeometryType::Polygon => Some(GeometryKind::Polygon),
    };
    const WGS84: &str = "GEOGCS[\"GCS_WGS_1984\",DATUM[\"D_WGS_1984\",SPHEROID[\"WGS_1984\",6378137.0,298.257223563]],PRIMEM[\"Greenwich\",0.0],UNIT[\"Degree\",0.0174532925199433]]";
    let mut w = if exists { GeodatabaseWriter::open_existing(gdb) } else { GeodatabaseWriter::create(gdb) }.map_err(bad)?;
    let mut t = w.create_table(&table, kind, false, false, &fields, kind.map(|_| WGS84)).map_err(bad)?;
    for f in &ds.features {
        let attributes = f
            .attrs
            .iter()
            .zip(&types)
            .map(|(v, ty)| match (v, ty) {
                (Value::Null, _) => GfValue::Null,
                (Value::Int(i), GfType::Integer) => GfValue::Integer(*i as i32),
                (Value::Int(i), _) => GfValue::Double(*i as f64),
                (Value::Float(x), _) => GfValue::Double(*x),
                (Value::Text(s), _) => GfValue::String(s.clone()),
                (Value::Bool(b), _) => GfValue::SmallInteger(i16::from(*b)),
            })
            .collect();
        let mut geometry = f.geometry.as_ref().and_then(to_unigis);
        // A class of multipoints holds its single points as one-point multipoints.
        if let Some(g) = geometry.as_mut().filter(|_| kind == Some(GeometryKind::Multipoint)) {
            g.kind = GeometryKind::Multipoint;
        }
        let geometry = geometry.filter(|g| Some(g.kind) == kind);
        t.write(&GfFeature { oid: 0, geometry, attributes }).map_err(bad)?;
    }
    w.finish_table(t).map_err(bad)?;
    w.finish().map_err(bad)?;
    Ok(ds.features.len())
}

/// A table's coordinate system, as [`FileGdbSource`] reads it.
fn reproject_of(table: &Table) -> Reproject {
    let mut notes = Vec::new();
    let crs = match table.geometry_def().and_then(|d| d.srs_wkt.as_deref()) {
        None => crate::raster::Crs::LonLat,
        Some(wkt) => crate::raster::wkt::read_wkt(wkt).map(|(c, _)| c).unwrap_or(crate::raster::Crs::LonLat),
    };
    Reproject::from_crs(crs, &mut notes)
}

/// A map value as a field of `ty` holds it; `Err` when it can't be (text in a number field).
fn to_field_value(v: &Value, ty: unigis_core::FieldType, field: &str) -> Result<GfValue> {
    use unigis_core::FieldType as G;
    let wrong = || bad(format!("{field}: {v:?} isn't a {}", type_name(ty)));
    let int = |v: &Value| -> Option<i64> {
        match v {
            Value::Int(i) => Some(*i),
            Value::Float(f) if f.fract() == 0.0 => Some(*f as i64),
            Value::Bool(b) => Some(i64::from(*b)),
            Value::Text(t) => t.trim().parse().ok(),
            Value::Float(_) | Value::Null => None,
        }
    };
    let float = |v: &Value| -> Option<f64> {
        match v {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Text(t) => t.trim().parse().ok(),
            _ => None,
        }
    };
    let text = |v: &Value| match v {
        Value::Text(t) => t.clone(),
        other => other.to_string(),
    };
    if matches!(v, Value::Null) {
        return Ok(GfValue::Null);
    }
    Ok(match ty {
        G::SmallInteger => GfValue::SmallInteger(int(v).and_then(|i| i16::try_from(i).ok()).ok_or_else(wrong)?),
        G::Integer => GfValue::Integer(int(v).and_then(|i| i32::try_from(i).ok()).ok_or_else(wrong)?),
        G::BigInteger => GfValue::BigInteger(int(v).ok_or_else(wrong)?),
        G::Single => GfValue::Single(float(v).ok_or_else(wrong)? as f32),
        G::Double => GfValue::Double(float(v).ok_or_else(wrong)?),
        G::String => GfValue::String(text(v)),
        G::Guid | G::GlobalId => GfValue::Guid(text(v)),
        G::Xml => GfValue::Xml(text(v)),
        G::Date => GfValue::Date(
            chrono::NaiveDateTime::parse_from_str(text(v).trim(), "%Y-%m-%d %H:%M:%S%.f")
                .or_else(|_| {
                    chrono::NaiveDate::parse_from_str(text(v).trim(), "%Y-%m-%d").map(|d| d.and_hms_opt(0, 0, 0).unwrap_or_default())
                })
                .map_err(|_| wrong())?,
        ),
        G::DateOnly => GfValue::DateOnly(chrono::NaiveDate::parse_from_str(text(v).trim(), "%Y-%m-%d").map_err(|_| wrong())?),
        G::TimeOnly => GfValue::TimeOnly(chrono::NaiveTime::parse_from_str(text(v).trim(), "%H:%M:%S%.f").map_err(|_| wrong())?),
        G::TimestampOffset => GfValue::TimestampOffset(chrono::DateTime::parse_from_rfc3339(text(v).trim()).map_err(|_| wrong())?),
        G::ObjectId | G::Geometry | G::Blob | G::Raster => return Err(wrong()),
    })
}

/// What a save of edits changed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Saved {
    pub inserted: usize,
    pub updated: usize,
    pub deleted: usize,
}

/// Saves a layer's features back into its feature class or table in place, row by row, as
/// ArcGIS's Save Edits does: rows whose OBJECTID (the feature's id) is gone are deleted, features
/// whose id the table doesn't have are inserted, and changed ones updated, with only the values
/// and geometry that changed rewritten, so the rest (dates, GUIDs, precision, domains, subtypes,
/// the schema itself) stay as stored. Fields are matched by name; ones the layer lacks keep
/// their values (or are null on insert).
pub fn save_edits(gdb_dir: &Path, table: &str, ds: &Dataset) -> Result<Saved> {
    let g = Geodatabase::open(gdb_dir).map_err(bad)?;
    let t = g.open_table(table).map_err(bad)?;
    let reproject = reproject_of(&t);
    let has_shape = t.geometry_kind().is_some();
    let base = g.table_base(table).map_err(bad)?;
    drop(t);
    let mut ed = unigis_filegdb::TableEditor::open(&base).map_err(bad)?;
    let fields: Vec<unigis_core::Field> = ed.fields().to_vec();
    // For each table field, the layer's column of that name.
    let column: Vec<Option<usize>> = fields.iter().map(|f| ds.fields.iter().position(|d| d.name.eq_ignore_ascii_case(&f.name))).collect();
    let ids: std::collections::HashSet<u64> = ds.features.iter().map(|f| f.id).collect();
    let mut saved = Saved::default();
    let gone: Vec<u64> = ed.oids().filter(|o| !ids.contains(o)).collect();
    for oid in gone {
        ed.delete(oid).map_err(bad)?;
        saved.deleted += 1;
    }
    let shape = |geom: &Option<Geometry>| -> Result<Option<unigis_core::Geometry>> {
        match geom {
            None => Ok(None),
            Some(g) if has_shape => {
                let native = reproject.geometry_from_lonlat(g.clone()).ok_or_else(|| bad("a geometry outside the coordinate system"))?;
                Ok(Some(to_unigis(&native).ok_or_else(|| bad("a geometry that can't be stored"))?))
            }
            Some(_) => Ok(None),
        }
    };
    for f in &ds.features {
        match ed.get(f.id).map_err(bad)? {
            Some(orig) => {
                let mut next = orig.clone();
                let mut changed = false;
                for (i, field) in fields.iter().enumerate() {
                    let (Some(j), Some(was)) = (column[i], orig.attributes.get(i)) else { continue };
                    let now = f.attrs.get(j).cloned().unwrap_or(Value::Null);
                    if value(was.clone()) != now {
                        next.attributes[i] = to_field_value(&now, field.field_type, &field.name)?;
                        changed = true;
                    }
                }
                let as_read = orig.geometry.as_ref().and_then(from_unigis).and_then(|g| reproject.geometry(g));
                if has_shape && as_read != f.geometry {
                    next.geometry = shape(&f.geometry)?;
                    changed = true;
                }
                if changed {
                    ed.update(f.id, &next).map_err(bad)?;
                    saved.updated += 1;
                }
            }
            None => {
                let mut attributes = Vec::with_capacity(fields.len());
                for (i, field) in fields.iter().enumerate() {
                    let v = column[i].and_then(|j| f.attrs.get(j)).cloned().unwrap_or(Value::Null);
                    attributes.push(to_field_value(&v, field.field_type, &field.name)?);
                }
                let feature = unigis_core::Feature { oid: 0, geometry: shape(&f.geometry)?, attributes };
                ed.insert(&feature).map_err(bad)?;
                saved.inserted += 1;
            }
        }
    }
    ed.finish().map_err(bad)?;
    Ok(saved)
}

/// ArcGIS's name for a field type (the Fields view's Data Type).
fn type_name(t: unigis_core::FieldType) -> &'static str {
    use unigis_core::FieldType as G;
    match t {
        G::SmallInteger => "Short",
        G::Integer => "Long",
        G::BigInteger => "Big Integer",
        G::Single => "Float",
        G::Double => "Double",
        G::String => "Text",
        G::Date => "Date",
        G::DateOnly => "Date Only",
        G::TimeOnly => "Time Only",
        G::TimestampOffset => "Timestamp Offset",
        G::ObjectId => "Object ID",
        G::Geometry => "Geometry",
        G::Blob => "Blob",
        G::Raster => "Raster",
        G::Guid => "Guid",
        G::GlobalId => "Global ID",
        G::Xml => "XML",
    }
}

/// A value as JSON text, for properties.
fn value_text(v: &GfValue) -> serde_json::Value {
    match value(v.clone()) {
        Value::Null => serde_json::Value::Null,
        Value::Int(i) => serde_json::json!(i),
        Value::Float(f) => serde_json::json!(f),
        Value::Bool(b) => serde_json::json!(b),
        Value::Text(s) => serde_json::json!(s),
    }
}

/// An item's properties, as ArcGIS's Properties and Fields views show them: for a table or
/// feature class its type, feature dataset, geometry (with Z/M), rows, extent, spatial reference,
/// fields (alias, type, length, nullable, default, domain), indexes, subtypes and the relationship
/// classes it takes part in; for a domain its type and values; for other items their type.
pub fn properties(gdb: &Geodatabase, name: &str) -> Result<serde_json::Value> {
    use serde_json::json;
    let name = name.trim();
    let item = gdb.items().iter().find(|i| i.name.eq_ignore_ascii_case(name));
    if let Some(d) = gdb.domain(name) {
        let values = match &d.kind {
            unigis_filegdb::DomainKind::Coded(codes) => json!({
                "type": "Coded Value",
                "codes": codes.iter().map(|(c, desc)| json!({"code": value_text(c), "description": desc})).collect::<Vec<_>>(),
            }),
            unigis_filegdb::DomainKind::Range { min, max } => json!({"type": "Range", "min": value_text(min), "max": value_text(max)}),
        };
        let mut out = json!({
            "name": d.name, "kind": "Domain", "description": d.description, "fieldType": type_name(d.field_type),
            "splitPolicy": format!("{:?}", d.split_policy), "mergePolicy": format!("{:?}", d.merge_policy),
        });
        if let (Some(o), Some(v)) = (out.as_object_mut(), values.as_object()) {
            o.extend(v.clone());
        }
        return Ok(out);
    }
    let Some(entry) = gdb.entries().iter().find(|e| e.name.eq_ignore_ascii_case(name)) else {
        let i = item.ok_or_else(|| bad(format!("no item {name}")))?;
        let mut out = json!({"name": i.name, "kind": i.kind, "path": i.path, "featureDataset": i.feature_dataset});
        add_relationship(gdb, name, &mut out);
        return Ok(out);
    };
    let t = gdb.open_table(&entry.name).map_err(bad)?;
    let kind = item.map_or_else(|| if t.geometry_kind().is_some() { "Feature Class" } else { "Table" }.to_string(), |i| i.kind.clone());
    let mut fields = Vec::new();
    if let Some(oid) = t.oid_field_name() {
        fields.push(json!({"name": oid, "type": "Object ID", "nullable": false}));
    }
    if let Some(shape) = t.shape_field() {
        fields.push(json!({"name": shape.name, "type": "Geometry", "nullable": shape.nullable}));
    }
    for f in t.fields() {
        fields.push(json!({
            "name": f.name, "alias": f.alias, "type": type_name(f.field_type), "length": f.length,
            "precision": f.precision, "scale": f.scale, "nullable": f.nullable,
            "default": f.default.as_ref().map(value_text), "domain": f.domain,
        }));
    }
    let mut out = json!({
        "name": entry.name, "kind": kind, "featureDataset": entry.feature_dataset, "rows": t.row_count(),
        "featureType": item.and_then(|i| i.feature_type.clone()),
        "fields": fields,
        "indexes": t.indexes().iter().map(|i| json!({"name": i.name, "fields": i.expression})).collect::<Vec<_>>(),
        "relationshipClasses": gdb.relationship_classes().iter()
            .filter(|r| r.origin.eq_ignore_ascii_case(&entry.name) || r.destination.eq_ignore_ascii_case(&entry.name))
            .map(|r| r.name.clone()).collect::<Vec<_>>(),
    });
    if let Some(k) = t.geometry_kind() {
        out["geometry"] = json!(format!("{k:?}"));
        out["hasZ"] = json!(t.has_z());
        out["hasM"] = json!(t.has_m());
    }
    if let Some(d) = t.geometry_def() {
        let e = &d.extent;
        if e.xmin.is_finite() && e.xmax.is_finite() && e.xmin <= e.xmax {
            out["extent"] = json!({"xmin": e.xmin, "ymin": e.ymin, "xmax": e.xmax, "ymax": e.ymax});
        }
        if let Some(wkt) = &d.srs_wkt {
            // The coordinate system's own name (`PROJCS["NAD_1983_UTM_Zone_10N", …`).
            let label = wkt.split('"').nth(1).unwrap_or("").to_string();
            out["spatialReference"] = json!({"name": label, "wkt": wkt});
        }
        out["xyTolerance"] = json!(d.xy_tolerance);
        out["xyResolution"] = json!(if d.xy_scale > 0.0 { 1.0 / d.xy_scale } else { 0.0 });
    }
    // An attributed relationship class has a table too.
    add_relationship(gdb, &entry.name, &mut out);
    if let Some(st) = t.subtypes() {
        out["subtypes"] = json!({
            "field": st.field, "default": st.default_code,
            "subtypes": st.subtypes.iter().map(|s| json!({"code": s.code, "name": s.name})).collect::<Vec<_>>(),
        });
    }
    Ok(out)
}

/// A relationship class's classes, keys and rules, when `name` is one.
fn add_relationship(gdb: &Geodatabase, name: &str, out: &mut serde_json::Value) {
    if let Some(r) = gdb.relationship_classes().iter().find(|r| r.name.eq_ignore_ascii_case(name)) {
        out["relationship"] = serde_json::json!({
            "origin": r.origin, "destination": r.destination, "cardinality": format!("{:?}", r.cardinality),
            "composite": r.is_composite, "attributed": r.is_attributed, "notification": r.notification,
            "originPrimaryKey": r.origin_primary_key, "originForeignKey": r.origin_foreign_key,
            "destinationPrimaryKey": r.destination_primary_key,
        });
    }
}

/// Reads a raster dataset of a geodatabase into the map's raster model: bands as 32-bit floats,
/// invalid pixels set to a NoData value (the type's own for floats, as ArcGIS shows them; for
/// integers a value no valid pixel has), three or four byte bands as RGB(A).
pub fn read_raster(gdb: &Geodatabase, name: &str) -> Result<crate::raster::Raster> {
    use crate::raster::{ColorHint, Crs, GeoTransform, Raster};
    let r = gdb.open_raster(name).map_err(bad)?;
    let mut bands = Vec::with_capacity(r.band_count());
    let mut any_invalid = false;
    let mut read = Vec::with_capacity(r.band_count());
    for b in 0..r.band_count() {
        let band = r.read_band(b).map_err(bad)?;
        any_invalid |= band.valid.is_some();
        read.push(band);
    }
    let nodata = any_invalid.then(|| {
        if r.pixel_type.is_float() {
            3.4e38
        } else {
            // The lowest value below every valid one (or above, for a type starting at 0).
            let min = read.iter().flat_map(|b| b.values.iter()).fold(f64::INFINITY, |m, v| m.min(*v));
            let max = read.iter().flat_map(|b| b.values.iter()).fold(f64::NEG_INFINITY, |m, v| m.max(*v));
            if min > 0.0 {
                0.0
            } else if min.is_finite() {
                (min - 1.0).max(-2_147_483_648.0)
            } else {
                max + 1.0
            }
        }
    });
    for band in read {
        let data = match (&band.valid, nodata) {
            (Some(valid), Some(nd)) => band.values.iter().zip(valid).map(|(v, ok)| if *ok { *v as f32 } else { nd as f32 }).collect(),
            _ => band.values.iter().map(|v| *v as f32).collect(),
        };
        bands.push(data);
    }
    let mut notes = Vec::new();
    let crs = match r.srs_wkt.as_deref() {
        None => Crs::LonLat,
        Some(wkt) => crate::raster::wkt::read_wkt(wkt)
            .map(|(c, note)| {
                notes.extend(note);
                c
            })
            .unwrap_or(Crs::LonLat),
    };
    let gt = r.geotransform;
    let transform = GeoTransform { origin_x: gt[0], origin_y: gt[3], pixel_width: gt[1], pixel_height: gt[5] };
    let byte = !r.pixel_type.is_float() && r.pixel_type.bits() <= 8;
    let color = match (bands.len(), byte) {
        (3, true) => ColorHint::Rgb { alpha: false },
        (4, true) => ColorHint::Rgb { alpha: true },
        _ => ColorHint::Gray,
    };
    let mut out =
        Raster::new(r.name.clone(), r.width as u32, r.height as u32, bands, transform, crs, nodata, color, r.pixel_type.arcgis_name())?;
    out.source = Some(format!("{}/{}", gdb.path().display(), r.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustgis_geom::Coord;

    fn reference() -> Option<std::path::PathBuf> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/gdb-reference/buffers.gdb");
        p.is_dir().then_some(p)
    }

    #[test]
    fn reads_a_feature_class_in_place() {
        let Some(p) = reference() else { return };
        let s = FileGdbSource::open(&p, Some("parcels_10")).unwrap();
        assert_eq!(s.info().geometry_type, GeometryType::Polygon);
        assert_eq!(s.info().format, "File Geodatabase");
        let all = s.read(&Request::all()).unwrap();
        assert!(all.complete);
        assert_eq!(all.features.len() as u64, s.info().count.unwrap());
        assert_eq!(all.skipped, 0);
        assert!(all.features.iter().all(|f| matches!(f.geometry, Some(Geometry::Polygon(_)))));
        assert!(all.features.iter().all(|f| f.attrs.len() == s.info().fields.len()));
        // The extent filter keeps the features meeting it and drops the rest.
        let e = s.info().extent;
        let corner = Extent::new(e.min_x, e.min_y, e.min_x + e.width() * 0.2, e.min_y + e.height() * 0.2);
        let part = s.read(&Request { extent: Some(corner), limit: 1000 }).unwrap();
        assert!(part.features.len() < all.features.len());
        for f in &part.features {
            assert!(f.geometry.as_ref().unwrap().extent().intersects(&corner));
        }
    }

    #[test]
    fn default_layer_is_the_first_feature_class() {
        let Some(p) = reference() else { return };
        let s = open_default(&p);
        assert_ne!(s.info().geometry_type, GeometryType::None);
    }

    fn open_default(p: &Path) -> FileGdbSource {
        FileGdbSource::open(p, None).unwrap()
    }

    fn parcels(name: &str, n: usize) -> Dataset {
        let mut d = Dataset::new(
            name,
            GeometryType::Polygon,
            vec![
                Field::new("Lot No", FieldType::Integer),
                Field::new("owner", FieldType::Text),
                Field::new("area", FieldType::Double),
                Field::new("Shape", FieldType::Text),
            ],
        );
        for i in 0..n {
            let (x, y) = (10.0 + i as f64 * 0.01, 50.0);
            let ring = vec![
                Coord::new(x, y),
                Coord::new(x, y + 0.005),
                Coord::new(x + 0.005, y + 0.005),
                Coord::new(x + 0.005, y),
                Coord::new(x, y),
            ];
            let hole = vec![
                Coord::new(x + 0.001, y + 0.001),
                Coord::new(x + 0.002, y + 0.001),
                Coord::new(x + 0.002, y + 0.002),
                Coord::new(x + 0.001, y + 0.002),
                Coord::new(x + 0.001, y + 0.001),
            ];
            let attrs = vec![
                Value::Int(i as i64 * 3_000_000_000),
                Value::Text(format!("Owner \u{e9} {i}")),
                Value::Float(i as f64 / 7.0),
                if i == 0 { Value::Null } else { Value::Text("s".into()) },
            ];
            d.push(Some(Geometry::Polygon(vec![vec![ring, hole]])), attrs);
        }
        d.push(None, vec![Value::Null, Value::Null, Value::Null, Value::Null]);
        d
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("rustgis-gdbw-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn written_layers_read_back_and_gdal_agrees() {
        let gdb = tmp("rt").join("out.gdb");
        let ds = parcels("My Parcels", 40);
        write(&ds, &gdb, None).unwrap();
        // Name and field names were made valid; a field too wide for 32 bits became a double.
        let s = FileGdbSource::open(&gdb, None).unwrap();
        assert_eq!(s.info().name, "My_Parcels");
        let names: Vec<&str> = s.info().fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Lot_No", "owner", "area", "Shape_"]);
        assert_eq!(s.info().fields[0].ty, FieldType::Double);
        let all = s.read(&Request::all()).unwrap();
        assert_eq!(all.features.len(), 41);
        let f = &all.features[3];
        assert_eq!(f.attrs[1], Value::Text("Owner \u{e9} 3".into()));
        assert!(matches!(f.attrs[2], Value::Float(x) if (x - 3.0 / 7.0).abs() < 1e-12));
        let Some(Geometry::Polygon(p)) = &f.geometry else { panic!("polygon") };
        assert_eq!((p.len(), p[0].len()), (1, 2), "one polygon with a hole");
        assert!(all.features[40].geometry.is_none());
        // GDAL reads what we wrote.
        if let Ok(out) = std::process::Command::new("ogrinfo").args(["-so", "-al"]).arg(&gdb).output()
            && out.status.success()
        {
            let t = String::from_utf8_lossy(&out.stdout);
            assert!(t.contains("Layer name: My_Parcels") && t.contains("Feature Count: 41") && t.contains("Lot_No: Real"), "{t}");
        }
        // Writing again adds a class; the same name replaces it.
        write(&parcels("Other", 5), &gdb, None).unwrap();
        write(&parcels("My Parcels", 7), &gdb, None).unwrap();
        let again = FileGdbSource::open(&gdb, Some("My_Parcels")).unwrap();
        assert_eq!(again.read(&Request::all()).unwrap().features.len(), 8);
        assert_eq!(FileGdbSource::open(&gdb, Some("Other")).unwrap().info().count, Some(6));
        let _ = std::fs::remove_dir_all(gdb.parent().unwrap());
    }

    #[test]
    fn paths_name_the_class_and_foreign_folders_are_left_alone() {
        let dir = tmp("path");
        std::fs::create_dir_all(&dir).unwrap();
        crate::io::write_path(&parcels("x", 2), &dir.join("g.gdb/roads")).unwrap();
        assert_eq!(FileGdbSource::open(&dir.join("g.gdb"), None).unwrap().info().name, "roads");
        std::fs::create_dir_all(dir.join("notes.gdb")).unwrap();
        std::fs::write(dir.join("notes.gdb/keep.txt"), "x").unwrap();
        assert!(write(&parcels("x", 2), &dir.join("notes.gdb"), None).is_err());
        assert!(dir.join("notes.gdb/keep.txt").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tables_and_points_are_written() {
        let gdb = tmp("tp").join("t.gdb");
        let mut t =
            Dataset::new("people", GeometryType::None, vec![Field::new("n", FieldType::Integer), Field::new("ok", FieldType::Boolean)]);
        t.push(None, vec![Value::Int(7), Value::Bool(true)]);
        write(&t, &gdb, None).unwrap();
        let mut p = Dataset::new("pts", GeometryType::Point, vec![]);
        p.push(Some(Geometry::Point(Coord::new(1.5, 2.5))), vec![]);
        p.push(Some(Geometry::MultiPoint(vec![Coord::new(3.0, 4.0), Coord::new(5.0, 6.0)])), vec![]);
        write(&p, &gdb, None).unwrap();
        let pts = FileGdbSource::open(&gdb, Some("pts")).unwrap().read(&Request::all()).unwrap();
        assert_eq!(pts.features.len(), 2);
        assert!(matches!(&pts.features[1].geometry, Some(Geometry::MultiPoint(v)) if v.len() == 2));
        let people = FileGdbSource::open(&gdb, Some("people")).unwrap();
        assert_eq!(people.info().geometry_type, GeometryType::None);
        let rows = people.read(&Request::all()).unwrap();
        assert_eq!(rows.features[0].attrs, vec![Value::Int(7), Value::Int(1)]);
        let _ = std::fs::remove_dir_all(gdb.parent().unwrap());
    }

    #[test]
    fn missing_layer_is_an_error_not_a_panic() {
        let Some(p) = reference() else { return };
        assert!(FileGdbSource::open(&p, Some("no_such_layer")).is_err());
        assert!(FileGdbSource::open(Path::new("/nonexistent/x.gdb"), None).is_err());
    }
}
