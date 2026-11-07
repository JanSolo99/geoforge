//! Desktop folders and data containers. Filesystem work and mtime checks stay off the frame.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};
use std::time::{Duration, SystemTime};
use web_time::Instant;

pub(crate) const PAGE_SIZE: usize = 100;
const CHECK_INTERVAL: Duration = Duration::from_secs(2);
const MAX_CACHED: usize = 16;
const MAX_PENDING: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Folder,
    Geodatabase,
    GeoPackage,
    Connection,
    FeatureDataset,
    FeatureClass,
    Table,
    Raster,
    Data,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Geodatabase => "Geodatabase",
            Self::GeoPackage => "GeoPackage",
            Self::Connection => "Database Connection",
            Self::FeatureDataset => "Feature Dataset",
            Self::FeatureClass => "Feature Class",
            Self::Table => "Table",
            Self::Raster => "Raster",
            Self::Data => "Data",
        }
    }

    pub fn navigable(self) -> bool {
        matches!(self, Self::Folder | Self::Geodatabase | Self::GeoPackage | Self::Connection | Self::FeatureDataset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub is_dir: bool,
    pub name: String,
    pub path: PathBuf,
    pub kind: Kind,
    pub layer: Option<String>,
    pub group: Option<String>,
    pub supported: bool,
}

impl Entry {
    pub fn file(is_dir: bool, name: String, path: PathBuf) -> Self {
        use rustgis_engine::data::raster;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let kind = match ext.as_str() {
            "gdb" => Kind::Geodatabase,
            "gpkg" => Kind::GeoPackage,
            "sde" | "geodatabase" => Kind::Connection,
            _ if is_dir => Kind::Folder,
            _ if raster::is_raster_name(&name) || raster::gdal::is_gdal_raster_name(&name) => Kind::Raster,
            "csv" | "tsv" | "txt" | "xlsx" | "xlsm" | "xlsb" | "xls" | "ods" => Kind::Table,
            "shp" | "geojson" | "json" | "fgb" | "parquet" | "geoparquet" | "gml" | "kml" | "gpx" => Kind::FeatureClass,
            _ => Kind::Data,
        };
        Self { is_dir, name, path, kind, layer: None, group: None, supported: true }
    }
}

#[derive(Default)]
struct Cached {
    entries: Option<Arc<Vec<Entry>>>,
    error: Option<String>,
    stamp: Option<SystemTime>,
    checked: Option<Instant>,
    generation: u64,
    dirty: bool,
    page: usize,
}

struct Listing {
    stamp: Option<SystemTime>,
    /// None means the directory timestamp has not changed.
    entries: Option<Arc<Vec<Entry>>>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Key {
    path: PathBuf,
    datasets: bool,
}

impl Key {
    fn new(path: &Path, datasets: bool) -> Self {
        Self { path: path.to_path_buf(), datasets }
    }
}

struct Pending {
    key: Key,
    generation: u64,
    answer: mpsc::Receiver<Result<Listing, String>>,
}

#[derive(Default)]
pub(crate) struct State {
    cache: HashMap<Key, Cached>,
    pending: Vec<Pending>,
}

pub(crate) struct View {
    pub entries: Option<Arc<Vec<Entry>>>,
    pub error: Option<String>,
    pub reading: bool,
    pub page: usize,
}

impl State {
    pub fn invalidate(&mut self) {
        for cached in self.cache.values_mut() {
            cached.generation = cached.generation.wrapping_add(1);
            cached.dirty = true;
        }
    }

    pub fn set_page(&mut self, path: &Path, page: usize) {
        if let Some(cached) = self.cache.get_mut(&Key::new(path, false)) {
            cached.page = page;
        }
    }

    pub fn contains_name(&self, path: &Path, name: &str) -> bool {
        self.cache
            .get(&Key::new(path, false))
            .and_then(|c| c.entries.as_ref())
            .is_some_and(|entries| entries.iter().any(|e| e.name.eq_ignore_ascii_case(name)))
    }

    fn poll(&mut self) {
        let mut i = 0;
        while i < self.pending.len() {
            let result = match self.pending[i].answer.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => {
                    i += 1;
                    continue;
                }
                Err(mpsc::TryRecvError::Disconnected) => Err("folder reader stopped".into()),
            };
            let pending = self.pending.swap_remove(i);
            if let Some(cached) = self.cache.get_mut(&pending.key)
                && cached.generation == pending.generation
            {
                cached.checked = Some(Instant::now());
                cached.dirty = false;
                match result {
                    Ok(listing) => {
                        cached.stamp = listing.stamp;
                        cached.error = None;
                        if let Some(entries) = listing.entries {
                            cached.page = cached.page.min(entries.len().saturating_sub(1) / PAGE_SIZE);
                            cached.entries = Some(entries);
                        }
                    }
                    Err(error) => {
                        cached.entries = None;
                        cached.error = Some(error);
                    }
                }
            }
        }
    }

    pub fn view(&mut self, path: &Path, ctx: &egui::Context) -> View {
        self.view_mode(path, false, ctx)
    }

    pub fn datasets(&mut self, path: &Path, ctx: &egui::Context) -> View {
        // Ordinary folders share the Catalog's exact listing. Containers get a
        // distinct key so their logical contents never replace filesystem entries.
        let logical = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ["gdb", "gpkg", "sde", "geodatabase"].iter().any(|kind| ext.eq_ignore_ascii_case(kind)));
        self.view_mode(path, logical, ctx)
    }

    fn view_mode(&mut self, path: &Path, datasets: bool, ctx: &egui::Context) -> View {
        self.poll();
        let key = Key::new(path, datasets);
        if !self.cache.contains_key(&key) && self.cache.len() >= MAX_CACHED {
            // Never evict an in-flight answer. At most two readers can be alive, even when
            // navigating rapidly or a network/NTFS directory is slow.
            let evict = self.cache.keys().find(|p| !self.pending.iter().any(|r| &r.key == *p)).cloned();
            if let Some(evict) = evict {
                self.cache.remove(&evict);
            }
        }
        let cached = self.cache.entry(key.clone()).or_default();
        let due = cached.dirty || cached.checked.is_none_or(|t| t.elapsed() >= CHECK_INTERVAL);
        if due && self.pending.len() < MAX_PENDING && !self.pending.iter().any(|p| p.key == key) {
            let (send, answer) = mpsc::channel();
            let dir = path.to_path_buf();
            let stamp = cached.stamp;
            let reuse = cached.entries.is_some() && !cached.dirty;
            let wake = ctx.clone();
            match std::thread::Builder::new().name("catalog-folder".into()).spawn(move || {
                let _ = send.send(read(&dir, stamp, reuse, datasets));
                wake.request_repaint();
            }) {
                Ok(_) => self.pending.push(Pending { key, generation: cached.generation, answer }),
                Err(error) => {
                    cached.error = Some(error.to_string());
                    cached.checked = Some(Instant::now());
                    cached.dirty = false;
                }
            }
        }
        let reading = cached.entries.is_none() && cached.error.is_none();
        // Poll mtimes without touching the filesystem on the UI thread. Also keep a path
        // waiting for one of the two reader slots moving even if no input arrives.
        ctx.request_repaint_after(if due { Duration::from_millis(100) } else { CHECK_INTERVAL });
        View { entries: cached.entries.clone(), error: cached.error.clone(), reading, page: cached.page }
    }
}

fn visible(name: &str, is_dir: bool) -> bool {
    use rustgis_engine::data::{io, mbtiles, photo, raster, source};
    if name.starts_with('.') || name.to_ascii_lowercase().ends_with(".aux.xml") {
        return false;
    }
    let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    is_dir
        || io::READ_EXTENSIONS.contains(&ext.as_str())
        || raster::is_raster_name(name)
        || raster::gdal::is_gdal_raster_name(name)
        || source::gdal::is_gdal_name(name)
        || source::duckdb::is_duckdb_name(name)
        || photo::is_photo_name(name)
        || mbtiles::is_mbtiles_name(name)
        || ext == "pmtiles"
        || ["sde", "geodatabase", "tsv", "rgis", "rgisx", "rgispkg", "rgismap", "rgismpk", "rgislyr", "rgislayout"].contains(&ext.as_str())
}

fn read(path: &Path, previous: Option<SystemTime>, reuse: bool, datasets: bool) -> Result<Listing, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let stamp_path = if datasets && ext == "gdb" { path.join("a00000004.gdbtable") } else { path.to_path_buf() };
    let stamp = std::fs::metadata(stamp_path).and_then(|m| m.modified()).ok();
    if reuse && stamp.is_some() && stamp == previous {
        return Ok(Listing { stamp, entries: None });
    }
    if datasets && matches!(ext.as_str(), "gdb" | "gpkg" | "sde" | "geodatabase") {
        return container(path, &ext).map(|entries| Listing { stamp, entries: Some(Arc::new(entries)) });
    }
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let is_dir = ty.is_dir() || (ty.is_symlink() && entry.path().is_dir());
        if visible(&name, is_dir) {
            entries.push(Entry::file(is_dir, name, entry.path()));
        }
    }
    entries.sort_by_cached_key(|e| (!e.is_dir, e.name.to_lowercase()));
    Ok(Listing { stamp, entries: Some(Arc::new(entries)) })
}

fn container(path: &Path, ext: &str) -> Result<Vec<Entry>, String> {
    use rustgis_engine::data::source::{filegdb, gpkg};
    let item = |name: String, kind, group, supported| Entry {
        is_dir: false,
        layer: Some(name.clone()),
        name,
        path: path.to_path_buf(),
        kind,
        group,
        supported,
    };
    let mut entries = match ext {
        "gdb" => {
            let gdb = filegdb::open_dir(path).map_err(|e| e.to_string())?;
            let contents = filegdb::contents(&gdb);
            let mut entries: Vec<_> = contents.feature_datasets.into_iter().map(|n| item(n, Kind::FeatureDataset, None, true)).collect();
            entries.extend(
                contents
                    .items
                    .into_iter()
                    .map(|i| item(i.name, if i.geometry.is_some() { Kind::FeatureClass } else { Kind::Table }, i.feature_dataset, true)),
            );
            entries.extend(
                contents
                    .others
                    .into_iter()
                    .filter(|i| i.kind == "Raster Dataset")
                    .map(|i| item(i.name, Kind::Raster, i.feature_dataset, true)),
            );
            entries
        }
        "gpkg" => gpkg::contents(path)
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|(name, ty)| {
                let kind = match ty.as_str() {
                    "features" => Kind::FeatureClass,
                    "attributes" => Kind::Table,
                    "tiles" | "2d-gridded-coverage" => Kind::Raster,
                    _ => Kind::Data,
                };
                item(name, kind, None, ty == "features")
            })
            .collect(),
        _ => return Err("This database connection format is not supported".into()),
    };
    entries.sort_by_cached_key(|e| (e.kind != Kind::FeatureDataset, e.name.to_lowercase()));
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path =
                std::env::temp_dir().join(format!("catalog-folders-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn catalog_folders_reads_all_pages_and_reuses_unchanged_listing() {
        let dir = Temp::new();
        for i in 0..401 {
            std::fs::write(dir.0.join(format!("image{i:03}.JP2")), []).unwrap();
        }
        std::fs::create_dir(dir.0.join("Z folder")).unwrap();
        std::fs::write(dir.0.join(".hidden.tif"), []).unwrap();
        std::fs::write(dir.0.join("image.jp2.aux.xml"), []).unwrap();
        let listing = read(&dir.0, None, false, false).unwrap();
        let entries = listing.entries.unwrap();
        assert_eq!(entries.len(), 402);
        assert!(entries[0].is_dir);
        assert_eq!(entries[401].name, "image400.JP2");
        assert!(read(&dir.0, listing.stamp, true, false).unwrap().entries.is_none());
        // A changed timestamp must reload; a forced refresh reloads even when it is equal.
        assert!(read(&dir.0, Some(SystemTime::UNIX_EPOCH), true, false).unwrap().entries.is_some());
        assert!(read(&dir.0, listing.stamp, false, false).unwrap().entries.is_some());
        for name in ["map.tif", "map.IMG", "map.sid", "map.ecw", "map.ntf", "map.dem", "map.bil", "map.gpkg", "map.fgb"] {
            assert!(visible(name, false), "{name}");
        }
        assert!(!visible("unsafe.vrt", false));
        assert!(!visible("map.img.aux.xml", false));
    }

    fn waiting(state: &mut State, path: &Path) -> mpsc::Sender<Result<Listing, String>> {
        let generation = state.cache.entry(Key::new(path, false)).or_default().generation;
        let (send, answer) = mpsc::channel();
        state.pending.push(Pending { key: Key::new(path, false), generation, answer });
        send
    }

    fn answer(name: &str) -> Listing {
        Listing { stamp: None, entries: Some(Arc::new(vec![Entry::file(false, name.into(), PathBuf::from(name))])) }
    }

    #[test]
    fn add_data_browser_containers_list_datasets_not_storage_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata");
        let listing = read(&root.join("gdb-reference/catalog_ops/before.gdb"), None, false, true).unwrap();
        let entries = listing.entries.unwrap();
        // ArcGIS-made catalog_ops fixture; GDAL's format listing also reports ds/inds,
        // loose (polygon), and tbl (without geometry).
        assert!(entries.iter().any(|e| e.name == "ds" && e.kind == Kind::FeatureDataset));
        assert!(entries.iter().any(|e| e.name == "inds" && e.group.as_deref() == Some("ds") && e.kind == Kind::FeatureClass));
        assert!(entries.iter().any(|e| e.name == "loose" && e.kind == Kind::FeatureClass));
        assert!(entries.iter().any(|e| e.name == "tbl" && e.kind == Kind::Table));
        assert!(!entries.iter().any(|e| e.name.starts_with("a000")));
        let tables = read(&root.join("gpkg/arcgis_tables.gpkg"), None, false, true).unwrap().entries.unwrap();
        assert_eq!(tables.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["parcels_tab", "roads_tab", "wells_tab"]);
        assert!(tables.iter().all(|e| e.kind == Kind::Table && !e.supported));
        assert!(container(Path::new("connection.sde"), "sde").unwrap_err().contains("not supported"));
    }

    #[test]
    fn add_data_browser_logical_contents_do_not_replace_catalog_filesystem_listing() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/gdb-reference/catalog_ops/before.gdb");
        let mut cache = State::default();
        let ctx = egui::Context::default();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let physical = cache.view(&path, &ctx);
            let logical = cache.datasets(&path, &ctx);
            if let (Some(physical), Some(logical)) = (physical.entries, logical.entries) {
                assert!(physical.iter().all(|e| e.layer.is_none()));
                assert!(logical.iter().any(|e| e.layer.as_deref() == Some("tbl")));
                assert!(!Arc::ptr_eq(&physical, &logical));
                assert_eq!(cache.cache.len(), 2);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn add_data_browser_keeps_file_types_offered_by_the_previous_picker() {
        for ext in [
            "geojson",
            "json",
            "csv",
            "tsv",
            "xlsx",
            "xlsm",
            "xlsb",
            "xls",
            "ods",
            "rss",
            "georss",
            "atom",
            "xml",
            "landxml",
            "jpg",
            "jpeg",
            "shp",
            "zip",
            "gpkg",
            "parquet",
            "geoparquet",
            "fgb",
            "duckdb",
            "ddb",
            "pmtiles",
            "mbtiles",
            "kml",
            "kmz",
            "gpx",
            "gml",
            "topojson",
            "dxf",
            "dwg",
            "tab",
            "mif",
            "osm",
            "pbf",
            "las",
            "laz",
            "tif",
            "tiff",
            "asc",
        ] {
            assert!(visible(&format!("data.{ext}"), false), "{ext}");
        }
        for ext in rustgis_engine::data::raster::gdal::EXTENSIONS {
            assert!(visible(&format!("data.{ext}"), false), "{ext}");
        }
    }

    #[test]
    fn catalog_folders_slow_reader_never_blocks_frames_or_overwrites_navigation() {
        let mut state = State::default();
        let a = Path::new("a");
        let b = Path::new("b");
        let slow = waiting(&mut state, a);
        let fast = waiting(&mut state, b);
        let ctx = egui::Context::default();
        // Neither reader has answered. Rendering must return without waiting or doing I/O.
        for _ in 0..100 {
            assert!(state.view(a, &ctx).reading);
            assert!(state.view(b, &ctx).reading);
        }
        fast.send(Ok(answer("b.tif"))).unwrap();
        assert_eq!(state.view(b, &ctx).entries.unwrap()[0].name, "b.tif");
        slow.send(Ok(answer("a.tif"))).unwrap();
        assert_eq!(state.view(b, &ctx).entries.unwrap()[0].name, "b.tif");
        assert_eq!(state.view(a, &ctx).entries.unwrap()[0].name, "a.tif");
        assert!(state.pending.is_empty());
    }

    #[test]
    fn catalog_folders_refresh_discards_old_answers_and_clamps_pages() {
        let mut state = State::default();
        let path = Path::new("refresh");
        let old = waiting(&mut state, path);
        state.invalidate();
        old.send(Ok(answer("old.tif"))).unwrap();
        state.poll();
        assert!(state.cache[&Key::new(path, false)].entries.is_none());
        let new = waiting(&mut state, path);
        state.set_page(path, 4);
        new.send(Ok(answer("created.tif"))).unwrap();
        let view = state.view(path, &egui::Context::default());
        assert_eq!(view.page, 0);
        assert_eq!(view.entries.unwrap()[0].name, "created.tif");
        assert!(state.contains_name(path, "CREATED.TIF"));
        let failed = waiting(&mut state, path);
        failed.send(Err("permission denied".into())).unwrap();
        let view = state.view(path, &egui::Context::default());
        assert!(view.entries.is_none());
        assert_eq!(view.error.as_deref(), Some("permission denied"));
    }
}
