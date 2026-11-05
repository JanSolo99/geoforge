//! Folders picked or dropped in the browser. A File Geodatabase is a folder (`Roads.gdb`) of many
//! files, so a page that only takes files can't add one: here a folder is walked, each geodatabase
//! in it (a folder holding `a00000001.gdbtable`, at any depth) is read, and its files go to the
//! app as one upload, `Roads.gdb`, an uncompressed zip archive of them that the geodatabase reader
//! opens in memory. Other files in a dropped folder are left out (and said to be).
//!
//! Modified for UniGIS (not part of upstream RustGIS).

use rustgis_ui_egui::rustgis_engine::cmd::raster::MAX_UPLOAD_BYTES;
use rustgis_ui_egui::rustgis_engine::data::zip;
use rustgis_ui_egui::{DATABASE_PICK, Inbox, i18n};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

use crate::parts::Notes;

/// Most files looked at in one folder, subfolders included.
const MAX_FILES: usize = 20_000;
/// Deepest subfolder walked.
const MAX_DEPTH: usize = 16;

/// A file in a picked or dropped folder, by its path in it (`Data/Roads.gdb/a00000001.gdbtable`).
pub struct Picked {
    pub path: String,
    pub file: web_sys::File,
}

/// The folder part of a path (`""` at the top).
fn folder(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

fn leaf(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The geodatabase folders among `files`, each with its own files (not those of its subfolders).
pub fn geodatabases(files: Vec<Picked>) -> (Vec<(String, Vec<Picked>)>, usize) {
    let mut gdbs: Vec<String> =
        files.iter().filter(|f| leaf(&f.path).eq_ignore_ascii_case("a00000001.gdbtable")).map(|f| folder(&f.path).to_string()).collect();
    gdbs.sort();
    gdbs.dedup();
    let mut out: Vec<(String, Vec<Picked>)> = gdbs.into_iter().map(|g| (g, Vec::new())).collect();
    let mut other = 0;
    for f in files {
        match out.iter_mut().find(|(g, _)| folder(&f.path) == g) {
            Some((_, list)) => list.push(f),
            None => other += 1,
        }
    }
    (out, other)
}

/// The name a geodatabase folder goes to the app under: its own, ending `.gdb`.
fn upload_name(folder: &str) -> String {
    let name = leaf(folder);
    let name = if name.is_empty() { "Geodatabase" } else { name };
    if name.to_ascii_lowercase().ends_with(".gdb") { name.to_string() } else { format!("{name}.gdb") }
}

/// Reads the geodatabases among a folder's files into the inbox, saying what happens in `notes`.
/// `mark` starts each name put in the inbox ([`DATABASE_PICK`] connects without adding layers).
pub async fn read(files: Vec<Picked>, mark: &'static str, inbox: Inbox, notes: Notes, ctx: egui::Context) {
    let note = |text: String, error: bool| notes.lock().unwrap_or_else(|e| e.into_inner()).push((text, error));
    let (gdbs, other) = geodatabases(files);
    if gdbs.is_empty() {
        note(i18n::fmt("No file geodatabase in that folder: a .gdb folder holds a00000001.gdbtable", &[]), true);
        ctx.request_repaint();
        return;
    }
    if other > 0 {
        note(i18n::fmt("Only geodatabases are read from a folder: {n} other files were left out", &[("n", &other.to_string())]), false);
    }
    for (path, list) in gdbs {
        let name = upload_name(&path);
        let size: f64 = list.iter().map(|f| f.file.size()).sum();
        if size > MAX_UPLOAD_BYTES as f64 {
            let mb = (MAX_UPLOAD_BYTES >> 20).to_string();
            note(
                i18n::fmt("{name} is larger than the web app's {mb} MB: open it in the desktop app", &[("name", &name), ("mb", &mb)]),
                true,
            );
            continue;
        }
        note(i18n::fmt("Reading {name}…", &[("name", &name)]), false);
        ctx.request_repaint();
        let mut entries = Vec::with_capacity(list.len());
        let mut failed = None;
        for f in list {
            match bytes(&f.file).await {
                Ok(b) => entries.push((leaf(&f.path).to_string(), b)),
                Err(e) => {
                    failed = Some(format!("{}: {e}", f.path));
                    break;
                }
            }
        }
        if let Some(e) = failed {
            note(e, true);
            continue;
        }
        match zip::write_stored(&entries) {
            Ok(archive) => inbox.lock().unwrap_or_else(|e| e.into_inner()).push((format!("{mark}{name}"), archive)),
            Err(e) => note(format!("{name}: {e}"), true),
        }
        ctx.request_repaint();
    }
    ctx.request_repaint();
}

/// A whole file's bytes.
async fn bytes(file: &web_sys::File) -> Result<Vec<u8>, String> {
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let buf = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await.map_err(err)?;
    let buf: js_sys::ArrayBuffer = buf.dyn_into().map_err(err)?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// Opens the browser's folder picker; the geodatabases in the folder picked go to the inbox, to be
/// connected to the project (Catalog › Add Database).
pub fn pick(inbox: Inbox, notes: Notes, ctx: egui::Context) -> Result<(), String> {
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
    let input: web_sys::HtmlInputElement = document.create_element("input").map_err(err)?.dyn_into().map_err(|_| "no input element")?;
    input.set_type("file");
    input.set_webkitdirectory(true);
    input.set_multiple(true);
    let picked = input.clone();
    let on_change = Closure::once(move |_: web_sys::Event| {
        let mut files = Vec::new();
        if let Some(list) = picked.files() {
            for i in 0..list.length().min(MAX_FILES as u32) {
                let Some(file) = list.get(i) else { continue };
                // `webkitRelativePath` names the file under the folder picked, the folder first.
                let path = js_sys::Reflect::get(&file, &"webkitRelativePath".into())
                    .ok()
                    .and_then(|v| v.as_string())
                    .filter(|p| !p.is_empty())
                    .unwrap_or_else(|| file.name());
                files.push(Picked { path, file });
            }
        }
        // Catalog › Add Database: connected, not added (as ArcGIS does).
        wasm_bindgen_futures::spawn_local(read(files, DATABASE_PICK, inbox, notes, ctx));
    });
    input.add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref()).map_err(err)?;
    on_change.forget();
    input.click();
    Ok(())
}

/// Listens for folders dropped on the page (before eframe, which only takes files, sees the
/// drop): their geodatabases go to the inbox. A drop of files alone is left to eframe.
pub fn listen_for_drops(inbox: Inbox, notes: Notes, ctx: egui::Context) -> Result<(), String> {
    let err = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let window = web_sys::window().ok_or("no window")?;
    let on_drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |e: web_sys::DragEvent| {
        let Some(items) = e.data_transfer().map(|d| d.items()) else { return };
        // Entries are only handed out while the event is being dispatched.
        let entries: Vec<web_sys::FileSystemEntry> =
            (0..items.length()).filter_map(|i| items.get(i)?.webkit_get_as_entry().ok().flatten()).collect();
        if !entries.iter().any(web_sys::FileSystemEntry::is_directory) {
            return;
        }
        e.prevent_default();
        e.stop_immediate_propagation();
        let (inbox, notes, ctx) = (inbox.clone(), notes.clone(), ctx.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let mut files = Vec::new();
            for entry in entries {
                if let Err(e) = walk(&entry, String::new(), 0, &mut files).await {
                    notes.lock().unwrap_or_else(|e| e.into_inner()).push((format!("{}: {e}", entry.name()), true));
                }
            }
            // Dropped on the map: its feature classes are added, as dragging one onto a map does.
            read(files, "", inbox, notes, ctx).await;
        });
    });
    window.add_event_listener_with_callback_and_bool("drop", on_drop.as_ref().unchecked_ref(), true).map_err(err)?;
    on_drop.forget();
    Ok(())
}

/// Collects the files under a dropped entry, by their paths from it.
async fn walk(entry: &web_sys::FileSystemEntry, base: String, depth: usize, out: &mut Vec<Picked>) -> Result<(), String> {
    let path = if base.is_empty() { entry.name() } else { format!("{base}/{}", entry.name()) };
    if out.len() >= MAX_FILES {
        return Ok(());
    }
    if entry.is_file() {
        let entry: &web_sys::FileSystemFileEntry = entry.unchecked_ref();
        let file = promise(|ok, fail| {
            entry.file_with_callback_and_callback(ok, fail);
            Ok(())
        })
        .await?;
        out.push(Picked { path, file: file.unchecked_into() });
        return Ok(());
    }
    if !entry.is_directory() || depth >= MAX_DEPTH {
        return Ok(());
    }
    let dir: &web_sys::FileSystemDirectoryEntry = entry.unchecked_ref();
    let reader = dir.create_reader();
    // `readEntries` hands out a directory a batch at a time, until an empty one.
    let mut children = Vec::new();
    loop {
        let batch = promise(|ok, fail| reader.read_entries_with_callback_and_callback(ok, fail)).await?;
        let batch: js_sys::Array = batch.unchecked_into();
        if batch.length() == 0 || children.len() >= MAX_FILES {
            break;
        }
        children.extend(batch.iter().map(|v| v.unchecked_into::<web_sys::FileSystemEntry>()));
    }
    for child in children {
        Box::pin(walk(&child, path.clone(), depth + 1, out)).await?;
    }
    Ok(())
}

/// Awaits a callback-style call (`call(success, failure)`).
async fn promise(
    call: impl FnOnce(&js_sys::Function, &js_sys::Function) -> Result<(), wasm_bindgen::JsValue>,
) -> Result<wasm_bindgen::JsValue, String> {
    let mut call = Some(call);
    let mut started = Ok(());
    let p = js_sys::Promise::new(&mut |ok, fail| {
        if let Some(call) = call.take() {
            started = call(&ok, &fail);
        }
    });
    started.map_err(|e| format!("{e:?}"))?;
    wasm_bindgen_futures::JsFuture::from(p).await.map_err(|e| format!("{e:?}"))
}
