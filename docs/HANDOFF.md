# Current handoff

Updated 2026-11-07T23:06:48.462145+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`; HEAD
`3db04580272bca2e51a036e4ce5003508688a7d8` (Browse data containers in the native Add Data dialog).

## Single item: app.add-data-browser — committed and pushed; complete

User authorizes one item through commit/push, then stop. Previous Catalog work
already accepted/landed; no old acceptance repeated. Read root/app guidance,
development skill, queue/priorities, roadmap/gaps. No agents/coordinator launched.

## Implementation

- New `app/crates/ui-egui/src/databrowser.rs`: native in-app Add Data and Add
  Geodatabase dialogs; logical FileGDB/GPKG items, feature datasets, Ctrl/Shift
  multi-select, type/name filters, 100-item pages, favorites/recent folders saved
  through preferences, Refresh and secondary OS Browse. Web upload flow retained.
- `folders.rs`: reuse Catalog's background worker/cache with typed entries;
  ordinary folders share listings, physical and logical containers have separate
  cache keys. Existing 16-cache/2-reader bounds and stale-generation checks retained.
  FileGDB catalog mtime drives contents refresh. All previous file-picker extensions
  retained using existing native/optional reader name predicates, including photos,
  tile archives, DuckDB and GDAL vectors. Auxiliary XML sidecars remain excluded.
- `data/src/source/gpkg.rs`: readonly gpkg_contents catalog API (10000-item cap).
  Unsupported attribute/tile readers and .sde/mobile connections are listed/reported
  honestly; reader work is not claimed complete. See KNOWN_GAPS.
- `engine/src/cmd/layer.rs`: explicitly selected nonspatial GDB tables use the
  existing standalone add_read path, preserving source and undo (were empty layers).
- UI integration, 8 translated strings in all15 catalogs, regressions, user guide,
  roadmap, priority tick and regenerated queue included. A final path safety adjustment uses the existing UI display-string convention
  before JSON conversion, preventing a serialization panic on non-UTF8 paths.
  No other item started.

## Evidence / debugging decisions

ArcGIS-made before.gdb fixture: GDAL format listing confirms ds/inds (point), loose
(polygon), tbl (nonspatial). arcgis_tables.gpkg has parcels_tab/roads_tab/wells_tab.
Metadata tests use these saved fixtures; no new geoprocessing algorithm or SDK use.
Initial full-UI failures exposed an unbounded separator hiding the footer, then the
selected-table backend bug; fixed both without weakening expected outcomes.
Review added physical/logical cache isolation and legacy-extension regressions.
An early root-workspace app-test invocation was rejected; corrected with manifest
path. Headless debug shot at1400px×2 exceeded existing2048 texture limit;1000px
shot revealed layout defect. First release build intentionally stopped (exit130)
for cache isolation. Subsequent final native builds passed.

## Validation

Sequential Cargo, -j3, `CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target`.
From app (or `--manifest-path app/Cargo.toml` from root):
- `cargo test -j3 -p rustgis-engine --features unigis
  geodatabase_selected_table_is_standalone -- --test-threads=1`: PASS1, including
  source/undo; `/tmp/add-data-table.log`.
- `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=1 cargo xtask ci`: EXIT1. Format/workspace
  clippy -D warnings PASS; engine122passed/6failed/1ignored. Same six existing plugin
  fixtures return `nope`; `/tmp/add-data-app-ci.{log,exit}`. Adapter gaps documented
  previously were not reached. No claim full app CI passes.
- Final `cargo clippy -j3 -p rustgis-ui-egui --features unigis --all-targets --
  -D warnings`: PASS; `/tmp/add-data-final-clippy.log`.
- Final `cargo test -j3 -p rustgis-ui-egui --features unigis -- --test-threads=1`:
  PASS51 unit +26 smoke; `/tmp/add-data-final-ui.log`. Covers metadata, cache modes,
  all previous picker extensions, filtering/selection/prefs, full UI navigation,
  multiple imports, connect/cancel and non-UTF8 filename failure safety. `cargo xtask layers`: PASS (no dependency edits
  afterwards); `/tmp/add-data-layers.log`.
- Final `cargo check -j3 -p rustgis-web --target wasm32-unknown-unknown`: PASS;
  `/tmp/add-data-web.log`. Final app fmt check and final native release build PASS.
From root: `CARGO_TARGET_DIR=... scripts/check`: PASS exit0;
`/tmp/add-data-root-check.{log,exit}`. Root code unchanged by subsequent UI-only
extension predicate adjustment; app checks above cover final UI. Pre-push runs the
full protected gate again. Protected verifier matches `/tmp/add-data-verifier.sha256`.

## Native desktop acceptance

Actual release app, isolated XDG_CONFIG_HOME `/tmp/add-data-ui-config`, cwd/fixtures
`/tmp/add-data-desktop-fixture`. PASS: GDB as one item, ds navigation, Ctrl multi-add
of loose+tbl (standalone table), TIFF raster filter/import, GPKG logical items and
nonempty points import; second.gdb connection leaves map/tables unchanged; .sde
shows unsupported message. Favorites/recent paths saved and restored after restart.
Final filename-only check shows all9 legacy format test names (not format-content
imports), `/tmp/add-data-desktop-final-formats.png`; favorites/restored recent folders
verified across an actual restart. Final path safety case verified in native window. JSON before/after snapshots verify live project state from own relay.
Screenshots inspected `/tmp/add-data-desktop-*.png`, result
`/tmp/add-data-desktop-result.json`, logs `/tmp/add-data-desktop{,-final}.log`.
X11 helper scopes input/capture to own config/PID. Palette Unicode/ambiguous search
was unsuitable; actual Add Data button and ribbon used. GUI PIDs2566392/2594421
stopped with SIGTERM (exit143); owned relay8767 closed/token removed after first
run, second run did not start a relay. No user configuration or source fixtures changed.

## Preservation / next actions

Inherited hashes `/tmp/add-data-inherited.json`: source/agent/reference edits all
unchanged; only generated dashboard refreshed independently. Untracked worktrees,
notes, research/fixtures and memory script untouched. No reset/stash/worktree transfer.
Only this item's app code/tests/translations/guide and root progress docs committed.
This operational handoff checkpoint is the only post-commit edit by this turn.

1. Native non-UTF8 path check PASS: selected a real invalid-byte filename; import
   reported an I/O error with application still responsive and no added layer.
   `/tmp/add-data-desktop-non-utf8.png`, `/tmp/add-data-desktop-path-safety.log`.
   Last GUI PID2604070 stopped (SIGTERM); no relay started in the last two runs.
   Landed `3db04580` on origin/main. Normal push passed the protected pre-push
   scripts/check (full fmt/lint/workspace tests and doc tests) and succeeded, exit0.
   Logs `/tmp/add-data-push.{log,exit}`; remote advanced `077f1c3c..3db04580`.
   Remote HEAD verified equal to local HEAD. No pending product work or tests.
2. This final post-push handoff checkpoint is intentionally uncommitted. Product,
   tests, translations, user guide, priority/queue/roadmap/gaps are committed/pushed.
   Generated dashboard remains dirty after hook; inherited source/untracked edits
   remain unstaged and untouched. No running GUI/build/test processes.
3. Next queue item after landing: app.basemap-gallery. Do not start it this turn.
