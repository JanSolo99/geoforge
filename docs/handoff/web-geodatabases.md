# Handoff: file geodatabases in the web app

Updated 2026-11-05 (Claude, cloud session), branch `ccr-c34e0864-7o9h7y` (based on main `850eb8f`).
Not landed on main. Another Claude may be working on main at the same time; this branch touches
`crates/unigis-filegdb`, `app/crates/{data,engine,ui-egui}`, `app/apps/rustgis-web` and docs.

## User objective

"Native gdb reading should be streamlined and easy, with gdbs SHOWING as gdbs just like arc
does" — in the browser too, where FileGDB reading was desktop-only.

## Done (this session)

1. `unigis-filegdb` reads from memory and builds for wasm32 (commit `312cd42`):
   `store.rs` (`Store::disk()` / `Store::memory(files)`, file names matched by last component,
   case-insensitive); `Geodatabase::from_files`, `Geodatabase::open_in`, `Table::open_in`.
   Unix `FileExt` calls go through `store::read_exact_at` / `write_all_at` (seek fallback off Unix).
   Fixed an overflow panic in `index::spatial_candidates` for windows wider than the index
   (cells clamped to the 31-bit key range). `tests/memory.rs`: 94 geodatabases / 309 tables read
   identically from memory and disk.
2. App data layer (`app/crates/data/src/source/filegdb.rs`, now built for wasm too):
   `zip_geodatabases` (finds `.gdb` folders in a zip at any depth, or files zipped without their
   folder), `zip_has_geodatabase`, `from_zip` (cached by the upload's `Arc`), `FileGdbSource::open_zip`,
   `layers_zip`, `contents` (`Contents`/`Item`). `source::open_shared` opens `Name.gdb` (bytes = zip of
   its files). `zip::write_stored` (no compression).
3. Engine: `layer.addDataBytes` with `Name.gdb` or a `.zip` holding geodatabases →
   `layer::add_geodatabases` (lists each in `Session::uploaded_geodatabases`, adds its feature
   classes; `catalogOnly`, `layer`). New commands `layer.addGeodatabaseItem`, `info.geodatabases`,
   `catalog.removeGeodatabase`. `Session::geodatabases()` = uploads + those read in place by the
   project's layers. Test `uploaded_geodatabase_is_listed_and_its_feature_classes_added`.
4. UI: Catalog › **Databases** (`panes.rs::databases`): each geodatabase with a drum icon, feature
   datasets (stacked icon) and classes/tables by geometry icon, double-click / Add To Map, context
   menu Add All Feature Classes / Remove From Catalog. Desktop also lists geodatabases its layers
   come from (uses the existing `geodatabase_tree`). `ui.addGeodatabase` (Home › Add › Geodatabase…,
   command search): browser folder picker / desktop folder dialog. Icons `geodatabase`,
   `feature_dataset`. 13 strings translated in all 15 catalogs.
5. Web (`apps/rustgis-web/src/folders.rs`): `<input webkitdirectory>` picker and a capture-phase
   `drop` listener walking dropped folders (`webkitGetAsEntry`); each `.gdb` folder → stored zip →
   inbox as `Name.gdb`. 256 MB limit (MAX_UPLOAD_BYTES on wasm).
6. `scripts/readiness/browser.cjs`: new check `add_file_geodatabase_zip` (uploads
   `testdata/gdb-reference/edit_fc.gdb` zipped; expects layer `wells` with source `edit_fc.gdb`).
7. Docs: ROADMAP Phase 3 line, READINESS matrix, KNOWN_GAPS row (web geodatabases).

## Validation (this branch, this container)

- `cargo test -p unigis-filegdb` all pass (oracle part skipped: no GDAL here; CI has it).
- `cargo clippy -p unigis-filegdb --all-targets` and for wasm32: clean.
- App: `cargo clippy -p rustgis-data -p rustgis-engine -p rustgis-ui-egui --features rustgis-ui-egui/unigis --all-targets -D warnings`
  clean; wasm32 `clippy -p rustgis-web -p rustgis-data --no-deps -D warnings` clean.
  (Without `--no-deps`, wasm clippy stops on an existing `drop(File)` lint in `engine/src/cmd/sharing.rs`.)
- `cargo test -p rustgis-data` 200 passed; engine `uploaded_geodatabase…` and `hostile_params_never_panic` pass.
- Tests with the `unigis` feature fail to LINK here (duplicate `geod_*` symbols: proj-sys and
  proj4rs-geodesic) — pre-existing, not caused by this branch; tests run without the feature.
- Build env notes: `RUSTGIS_DATA_DIR` pointed at the assets fetched from
  `raw.githubusercontent.com/opengeos/rustgis-assets/main/data/` (assets.rustgis.app is blocked
  here); `apt-get install sqlite3 libwayland-dev libxkbcommon-dev libgtk-3-dev libudev-dev`.

## Next

- Real-browser run of `browser.cjs` against a fresh `scripts/build-web.sh` build (see final report
  of the session for whether it ran).
- Optional: keep uploaded geodatabases in the saved project (currently only via streamed layers).
- Land on main via a PR (user decides); the Netlify workflow then runs the browser gate.
