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

- Root `cargo clippy --locked --workspace --all-targets -D warnings` clean; root `cargo test --locked --workspace --no-fail-fast` all pass with GDAL 3.8.4 installed (`apt-get install gdal-bin`), incl. the FileGDB oracle and `tests/memory.rs`.
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

- Real browser: `scripts/build-web.sh` (release, trunk 0.21.14) built `app/dist/web` (31 MB) from
  this branch; `node scripts/readiness/browser.cjs --dist app/dist/web` passed all 6 checks
  (incl. `add_file_geodatabase_zip`), zero page errors, Chromium from the container.
  Catalog screenshots (scratch harness) show Databases › testopenfilegdb.gdb (33 items, geometry
  icons) and featuredataset.gdb (feature datasets fd1, fd2 + standalone).
- Harness note: at the top level, `?embed` makes the app its own parent, so it receives its own
  `project` messages and reopens the project (which clears the uploaded-geodatabase list, as
  opening a project should). Real embeddings (an iframe) don't. The scratch screenshot harness
  intercepts `window.postMessage` for that reason.

## Second pass (user: "fix all those on the branch")

Commits after `1518b16`, all pushed: projects keep databases once (`1d99b5b`), item types (`66db3f2`),
Properties (`45c008f`), MultiPatch/annotation + link fix (`8f1d71c`), raster datasets read
(`052ffc2`, `a1d7a2e`), 1 GiB uploads held once (`5408625`), vfs (`0df8ac0`), browser editing
(`7b4e801`). Status per item: `docs/ROADMAP.md` › Phase 3 › "Geodatabases in the app".

Validation of this pass: unigis-filegdb all tests (incl. tests/raster.rs vs GDAL 3.8: 50 rasters,
1.38 M pixels; tests/memory.rs incl. byte-identical in-memory edits); rustgis-data 204+; engine 121
with `--features unigis` (links now); clippy -D warnings native + wasm32; release web build +
`browser.cjs` 8/8 checks, zero page errors (zip upload, folder picker, download).

Build-environment notes: use `CARGO_INCREMENTAL=0` and one feature set (`--features
rustgis-ui-egui/unigis`) or the 40 GB disk fills; `apt-get install gdal-bin` for the oracle tests.

## Next

Open (also in ROADMAP/KNOWN_GAPS): in-place row/schema edits (TableEditor), raster pyramids /
windowed reads / RAT / write, >1 GiB uploads via a Web Worker, per-feature annotation, Copy/Paste /
Import / Compact / Recover / metadata. Blocked here (need ArcGIS-made samples and the bridge):
`.cdf` tables, mobile geodatabases. Then a PR (user decides).
