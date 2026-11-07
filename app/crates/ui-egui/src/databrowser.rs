//! Native Add Data: browse logical datasets rather than the files inside a geodatabase.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::vec2;
use serde_json::{Value, json};

use crate::folders::{Entry, Kind, PAGE_SIZE};
use crate::{Pane, RustGisApp};

#[derive(Clone, Default, PartialEq, Eq)]
struct Location {
    path: PathBuf,
    group: Option<String>,
}

impl Location {
    fn from_path(path: PathBuf) -> Self {
        if let Some(db) = path.ancestors().find(|p| extension(p) == "gdb") {
            let group = path.strip_prefix(db).ok().filter(|p| !p.as_os_str().is_empty()).map(|p| p.to_string_lossy().into_owned());
            Self { path: db.to_path_buf(), group }
        } else {
            Self { path, group: None }
        }
    }

    fn address(&self) -> String {
        self.group.as_ref().map_or_else(|| self.path.clone(), |g| self.path.join(g)).display().to_string()
    }

    fn up(&self) -> Option<Self> {
        if self.group.is_some() {
            Some(Self { path: self.path.clone(), group: None })
        } else {
            self.path.parent().map(|p| Self { path: p.to_path_buf(), group: None })
        }
    }
}

fn extension(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Features,
    Tables,
    Raster,
}

impl Filter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "All files",
            Self::Features => "Features",
            Self::Tables => "Tables",
            Self::Raster => "Raster",
        }
    }

    fn matches(self, e: &Entry) -> bool {
        e.kind.navigable()
            || match self {
                Self::All => true,
                Self::Features => matches!(e.kind, Kind::FeatureClass | Kind::Data),
                Self::Tables => e.kind == Kind::Table,
                Self::Raster => e.kind == Kind::Raster,
            }
    }
}

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    connect: bool,
    location: Location,
    address: String,
    favourites: Vec<PathBuf>,
    recent: Vec<PathBuf>,
    loaded_prefs: bool,
    selected: Vec<Entry>,
    anchor: Option<usize>,
    filter: Filter,
    search: String,
    page: usize,
    listing: Option<Arc<Vec<Entry>>>,
    indices: Vec<usize>,
    dirty: bool,
    error: Option<String>,
}

fn pref_paths(app: &RustGisApp, key: &str) -> Vec<PathBuf> {
    app.services
        .load_pref
        .as_ref()
        .and_then(|load| load(key))
        .filter(|s| s.len() <= 65_536)
        .and_then(|s| serde_json::from_str::<Vec<PathBuf>>(&s).ok())
        .unwrap_or_default()
        .into_iter()
        .take(32)
        .collect()
}

impl State {
    fn navigate(&mut self, location: Location) {
        self.location = location;
        self.address = self.location.address();
        self.selected.clear();
        self.anchor = None;
        self.listing = None;
        self.indices.clear();
        self.page = 0;
        self.dirty = true;
        self.error = None;
    }

    fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(16);
    }

    fn filter_entries(&mut self, entries: Arc<Vec<Entry>>) {
        let changed = self.listing.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &entries));
        if changed || self.dirty {
            let query = self.search.to_lowercase();
            self.indices = entries
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    e.group == self.location.group
                        && self.filter.matches(e)
                        && (query.is_empty() || e.name.to_lowercase().contains(&query))
                        && (!self.connect || e.kind.navigable())
                        && !rustgis_engine::project::is_project_file(&e.name)
                        && !e.name.to_ascii_lowercase().starts_with(".rgis")
                })
                .map(|(i, _)| i)
                .collect();
            self.page = self.page.min(self.indices.len().saturating_sub(1) / PAGE_SIZE);
            self.selected.retain(|e| self.indices.iter().any(|i| entries.get(*i) == Some(e)));
            self.anchor = None;
            self.listing = Some(entries);
            self.dirty = false;
        }
    }

    fn select(&mut self, at: usize, entry: &Entry, modifiers: egui::Modifiers) {
        if modifiers.shift
            && let Some(anchor) = self.anchor
        {
            if !modifiers.command && !modifiers.ctrl {
                self.selected.clear();
            }
            for i in anchor.min(at)..=anchor.max(at) {
                if let Some(e) = self.indices.get(i).and_then(|i| self.listing.as_ref()?.get(*i))
                    && !e.kind.navigable()
                    && e.supported
                    && !self.selected.contains(e)
                {
                    self.selected.push(e.clone());
                }
            }
        } else {
            if !modifiers.command && !modifiers.ctrl {
                self.selected.clear();
            }
            if let Some(i) = self.selected.iter().position(|e| e == entry) {
                self.selected.remove(i);
            } else {
                self.selected.push(entry.clone());
            }
            self.anchor = Some(at);
        }
    }
}

pub(crate) fn open(app: &mut RustGisApp, connect: bool, path: Option<&str>) {
    let mut state = std::mem::take(&mut app.ui.data_browser);
    if !state.loaded_prefs {
        state.favourites = pref_paths(app, "data_favourites");
        state.recent = pref_paths(app, "data_recent");
        state.loaded_prefs = true;
    }
    let initial = path
        .map(PathBuf::from)
        .or_else(|| app.table.folder.clone())
        .or_else(|| (!state.location.path.as_os_str().is_empty()).then(|| state.location.path.clone()))
        .or_else(|| state.recent.first().cloned())
        .or_else(|| app.session.path.as_ref().and_then(|p| p.parent().map(Path::to_path_buf)))
        .or_else(|| std::env::current_dir().ok());
    if let Some(path) = initial {
        state.navigate(Location::from_path(path));
    }
    state.connect = connect;
    state.open = true;
    app.ui.data_browser = state;
}

fn save(app: &RustGisApp, state: &State) {
    if let Some(save) = &app.services.save_pref {
        if let Ok(value) = serde_json::to_string(&state.favourites) {
            save("data_favourites", &value);
        }
        if let Ok(value) = serde_json::to_string(&state.recent) {
            save("data_recent", &value);
        }
    }
}

fn location_for(entry: &Entry) -> Location {
    Location { path: entry.path.clone(), group: (entry.kind == Kind::FeatureDataset).then(|| entry.name.clone()) }
}

/// Runs the existing add commands, retaining failed selections so a partial multi-add can
/// be retried without duplicating the items that already succeeded.
fn add(app: &mut RustGisApp, entries: &[Entry]) -> Vec<(Entry, String)> {
    let mut failed = Vec::new();
    for e in entries {
        let result = if !e.supported || e.kind.navigable() {
            Err(tl!("This dataset type cannot be added yet").to_string())
        } else if extension(&e.path) == "gdb" {
            app.run("catalog.addDatabase", json!({"path": e.path.display().to_string()})).and_then(|db| {
                let name = db.get("geodatabase").and_then(Value::as_str).ok_or("database name missing")?;
                app.run("layer.addGeodatabaseItem", json!({"geodatabase": name, "layer": e.layer, "name": e.name}))
            })
        } else {
            let mut params = json!({"path": e.path.display().to_string()});
            if let Some(layer) = &e.layer {
                params["layer"] = json!(layer);
                params["name"] = json!(e.name);
            }
            app.run("layer.addData", params)
        };
        if let Err(error) = result {
            failed.push((e.clone(), error));
        }
    }
    failed
}

pub(crate) fn show(app: &mut RustGisApp, ctx: &egui::Context) {
    if !app.ui.data_browser.open {
        return;
    }
    let mut state = std::mem::take(&mut app.ui.data_browser);
    let mut persist = false;
    let view = app.ui.folders.datasets(&state.location.path, ctx);
    if let Some(entries) = view.entries {
        state.filter_entries(entries);
        if state.location.group.is_none()
            && !matches!(extension(&state.location.path).as_str(), "gdb" | "gpkg" | "sde" | "geodatabase")
            && state.recent.first() != Some(&state.location.path)
        {
            state.remember(state.location.path.clone());
            persist = true;
        }
    }
    let mut open = true;
    let mut cancel = false;
    let mut navigate = None;
    let mut add_entries = None;
    let mut connect_path = None;
    let title = if state.connect { tl!("Add Geodatabase…") } else { tl!("Add Data") };
    egui::Window::new(title)
        .id(egui::Id::new("add-data-browser"))
        .open(&mut open)
        .default_size(vec2(850.0, 580.0))
        .min_size(vec2(600.0, 420.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.add_enabled(state.location.up().is_some(), egui::Button::new("↑")).on_hover_text(tl!("Parent folder")).clicked() {
                    navigate = state.location.up();
                }
                ui.label(tl!("Location"));
                let address = ui.add(egui::TextEdit::singleline(&mut state.address).desired_width(ui.available_width() - 130.0));
                let enter = address.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button(tl!("Go")).clicked() || enter {
                    navigate = Some(Location::from_path(PathBuf::from(state.address.trim())));
                }
                if ui.button(tl!("Refresh")).clicked() {
                    app.ui.folders.invalidate();
                }
            });
            ui.separator();
            let panel_height = (ctx.content_rect().height() - 220.0).clamp(240.0, 440.0);
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(185.0);
                    egui::ScrollArea::vertical().id_salt("add-data-places").max_height(panel_height).show(ui, |ui| {
                        ui.strong(tl!("Favorites"));
                        let folder = state.location.path.parent().map(Path::to_path_buf).unwrap_or_else(|| state.location.path.clone());
                        let current = if matches!(extension(&state.location.path).as_str(), "gdb" | "gpkg" | "sde" | "geodatabase") {
                            folder
                        } else {
                            state.location.path.clone()
                        };
                        let favourite = state.favourites.contains(&current);
                        if ui.button(tl!(if favourite { "Remove from favorites" } else { "Add to favorites" })).clicked() {
                            if favourite {
                                state.favourites.retain(|p| p != &current);
                            } else if state.favourites.len() < 32 {
                                state.favourites.push(current);
                            }
                            persist = true;
                        }
                        for path in &state.favourites {
                            if ui.link(path.display().to_string()).clicked() {
                                navigate = Some(Location::from_path(path.clone()));
                            }
                        }
                        ui.separator();
                        ui.strong(tl!("Recent folders"));
                        for path in &state.recent {
                            if ui.link(path.display().to_string()).clicked() {
                                navigate = Some(Location::from_path(path.clone()));
                            }
                        }
                        ui.separator();
                        ui.strong(tl!("Folder Connections"));
                        for path in &app.session.project.folders {
                            if ui.link(path).clicked() {
                                navigate = Some(Location::from_path(PathBuf::from(path)));
                            }
                        }
                    });
                });
                ui.add_sized(vec2(8.0, panel_height), egui::Separator::default().vertical());
                ui.vertical(|ui| {
                    ui.set_min_width(350.0);
                    ui.horizontal(|ui| {
                        ui.label(tl!("Type"));
                        egui::ComboBox::from_id_salt("add-data-type").selected_text(tl!(state.filter.label())).show_ui(ui, |ui| {
                            for filter in [Filter::All, Filter::Features, Filter::Tables, Filter::Raster] {
                                if ui.selectable_value(&mut state.filter, filter, tl!(filter.label())).changed() {
                                    state.dirty = true;
                                    state.page = 0;
                                }
                            }
                        });
                        if ui.add(egui::TextEdit::singleline(&mut state.search).hint_text(tl!("Search")).desired_width(150.0)).changed() {
                            state.dirty = true;
                            state.page = 0;
                        }
                    });
                    if view.reading {
                        ui.label(tl!("Reading…"));
                    }
                    if let Some(error) = &view.error {
                        ui.colored_label(ui.visuals().error_fg_color, tl!(error.as_str()));
                    }
                    let rows: Vec<(usize, Entry)> = state
                        .indices
                        .iter()
                        .enumerate()
                        .skip(state.page * PAGE_SIZE)
                        .take(PAGE_SIZE)
                        .filter_map(|(at, i)| Some((at, state.listing.as_ref()?.get(*i)?.clone())))
                        .collect();
                    egui::ScrollArea::vertical()
                        .id_salt("add-data-items")
                        .max_height(panel_height - 80.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            egui::Grid::new("add-data-grid").striped(true).min_col_width(230.0).show(ui, |ui| {
                                ui.strong(tl!("Name"));
                                ui.strong(tl!("Type"));
                                ui.end_row();
                                for (at, entry) in rows {
                                    let response = ui.selectable_label(state.selected.contains(&entry), &entry.name);
                                    if response.clicked() {
                                        state.select(at, &entry, ui.input(|i| i.modifiers));
                                    }
                                    if response.double_clicked() {
                                        if entry.kind.navigable() {
                                            navigate = Some(location_for(&entry));
                                        } else if entry.supported {
                                            add_entries = Some(vec![entry.clone()]);
                                        }
                                    }
                                    if !entry.supported {
                                        response.on_hover_text(tl!("This dataset type cannot be added yet"));
                                    }
                                    ui.label(tl!(entry.kind.label()));
                                    ui.end_row();
                                }
                            });
                        });
                    let pages = state.indices.len().div_ceil(PAGE_SIZE).max(1);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(state.page > 0, egui::Button::new(tl!("Previous Page"))).clicked() {
                            state.page = state.page.saturating_sub(1);
                        }
                        ui.label(format!("{} / {}", state.page + 1, pages));
                        if ui.add_enabled(state.page + 1 < pages, egui::Button::new(tl!("Next Page"))).clicked() {
                            state.page += 1;
                        }
                    });
                });
            });
            ui.separator();
            if let Some(error) = &state.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                if ui.button(tl!("Browse…")).clicked() {
                    if state.connect {
                        if let Some(path) = rfd::FileDialog::new().set_title(title).pick_folder() {
                            navigate = Some(Location::from_path(path));
                        }
                    } else if let Some(path) = rfd::FileDialog::new().set_title(title).pick_file() {
                        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                        let entry = Entry::file(false, name, path);
                        if entry.kind.navigable() {
                            navigate = Some(location_for(&entry));
                        } else {
                            if let Some(parent) = entry.path.parent() {
                                navigate = Some(Location::from_path(parent.to_path_buf()));
                            }
                            add_entries = Some(vec![entry]);
                        }
                    }
                }
                let one = (state.selected.len() == 1).then(|| state.selected.first()).flatten();
                let browse = one.filter(|e| e.kind.navigable());
                let db = one
                    .filter(|e| e.kind == Kind::Geodatabase)
                    .map(|e| e.path.clone())
                    .or_else(|| (extension(&state.location.path) == "gdb").then(|| state.location.path.clone()));
                let enabled = if state.connect {
                    db.is_some() || browse.is_some()
                } else {
                    browse.is_some() || (!state.selected.is_empty() && state.selected.iter().all(|e| e.supported && !e.kind.navigable()))
                };
                let label = if browse.is_some() && (!state.connect || db.is_none()) { tl!("Open") } else { tl!("Add") };
                if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                    if state.connect && db.is_some() {
                        connect_path = db;
                    } else if let Some(entry) = browse {
                        navigate = Some(location_for(entry));
                    } else {
                        add_entries = Some(state.selected.clone());
                    }
                }
                if ui.button(tl!("Cancel")).clicked() {
                    cancel = true;
                }
            });
        });
    if let Some(location) = navigate {
        if location.group.is_none() && !matches!(extension(&location.path).as_str(), "gdb" | "gpkg" | "sde" | "geodatabase") {
            state.remember(location.path.clone());
            persist = true;
        }
        state.navigate(location);
    }
    if let Some(path) = connect_path {
        match app.run("catalog.addDatabase", json!({"path": path.display().to_string()})) {
            Ok(_) => {
                open = false;
                app.ui.dock.open(Pane::Catalog);
            }
            Err(error) => state.error = Some(error),
        }
    }
    if let Some(entries) = add_entries {
        let failures = add(app, &entries);
        state.selected = failures.iter().map(|(e, _)| e.clone()).collect();
        state.error = failures.into_iter().map(|(_, e)| e).next();
        if state.error.is_none() {
            open = false;
        }
        let parent = if extension(&state.location.path) == "gdb" || extension(&state.location.path) == "gpkg" {
            state.location.path.parent().map(Path::to_path_buf)
        } else {
            Some(state.location.path.clone())
        };
        if let Some(parent) = parent {
            state.remember(parent);
            persist = true;
        }
    }
    if persist {
        save(app, &state);
    }
    state.open = open && !cancel;
    app.ui.data_browser = state;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn add_data_browser_non_utf8_path_fails_without_panicking() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(std::ffi::OsString::from_vec(b"/missing/data-\xff.geojson".to_vec()));
        let entry = Entry::file(false, "data.geojson".into(), path);
        let mut app = RustGisApp::new(rustgis_engine::Session::new(Default::default()));
        assert_eq!(add(&mut app, &[entry]).len(), 1);
        assert!(app.session.map().layers.is_empty());
    }

    #[test]
    fn add_data_browser_filters_keep_containers_and_selection_handles_control_and_shift() {
        let entries = Arc::new(vec![
            Entry::file(true, "data.gdb".into(), PathBuf::from("data.gdb")),
            Entry::file(false, "roads.shp".into(), PathBuf::from("roads.shp")),
            Entry::file(false, "values.csv".into(), PathBuf::from("values.csv")),
            Entry::file(false, "image.JP2".into(), PathBuf::from("image.JP2")),
        ]);
        let mut state = State::default();
        state.filter_entries(entries.clone());
        state.select(1, &entries[1], egui::Modifiers::NONE);
        state.select(2, &entries[2], egui::Modifiers::CTRL);
        assert_eq!(state.selected.len(), 2);
        state.select(3, &entries[3], egui::Modifiers::SHIFT);
        assert_eq!(state.selected.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["values.csv", "image.JP2"]);
        state.filter = Filter::Raster;
        state.dirty = true;
        state.filter_entries(entries.clone());
        assert_eq!(state.indices, [0, 3]);
        assert_eq!(state.selected.len(), 1);
        state.filter = Filter::Tables;
        state.dirty = true;
        state.filter_entries(entries);
        assert_eq!(state.indices, [0, 2]);
        assert!(state.selected.is_empty());
        state.navigate(Location::from_path(PathBuf::from("data.gdb/ds")));
        assert_eq!(state.location.group.as_deref(), Some("ds"));
        assert_eq!(state.location.up().unwrap().path, PathBuf::from("data.gdb"));
        assert!(state.location.up().unwrap().group.is_none());
    }

    #[test]
    fn add_data_browser_saves_favourites_and_recent_folders_without_duplicates() {
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;
        let prefs = Rc::new(RefCell::new(HashMap::<String, String>::new()));
        let read = prefs.clone();
        let write = prefs.clone();
        let mut app = RustGisApp::with_sample();
        app.services.load_pref = Some(Box::new(move |key| read.borrow().get(key).cloned()));
        app.services.save_pref = Some(Box::new(move |key, value| {
            write.borrow_mut().insert(key.to_string(), value.to_string());
        }));
        let mut state = State { favourites: vec![PathBuf::from("/my/data")], ..State::default() };
        state.remember(PathBuf::from("/old"));
        state.remember(PathBuf::from("/new"));
        state.remember(PathBuf::from("/old"));
        save(&app, &state);
        open(&mut app, false, Some("/my/data"));
        assert_eq!(app.ui.data_browser.favourites, [PathBuf::from("/my/data")]);
        assert_eq!(app.ui.data_browser.recent, [PathBuf::from("/old"), PathBuf::from("/new")]);
        app.services.load_pref = Some(Box::new(|_| Some("[not json".into())));
        assert!(pref_paths(&app, "data_recent").is_empty());
    }
}
