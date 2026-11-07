# Current handoff

Updated 2026-11-07T21:49:52.857332+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`; HEAD before landing
`f3181291f4579b17604f6964ce992ad9bb89ac37`.

## Single item: app.catalog-no-freeze — complete, landing now

User authorizes commit/push of one item, then stop. Previous gp-empty-map was
already committed; this turn selected the next app priority. No agents launched.
Read AGENTS, CLAUDE, development skill, app/AGENTS, queue/priorities, roadmap/gaps.

## Changes in this turn

- `app/crates/ui-egui/src/folders.rs`: native background listing/mtime checks
  (2s interval), 16 cached folders, at most 2 active readers, immutable listing
  snapshots, stale-generation rejection, errors and page state. All filesystem
  enumeration/stat work stays off the frame.
- `lib.rs`, `panes.rs`, `chrome.rs`: Reading… indicator, Refresh command/button,
  100-item pages instead of truncation at 300, invalidation after successful
  GP/catalog commands, asynchronous name checks in New menus. Shared Add Data
  raster/vector extension predicates; auxiliary XML sidecars excluded. Native-only
  code guarded for web. Reading… translated in all 15 catalogs.
- Three cache regressions and a full-UI paging/refresh/mtime regression. Initial
  test compile fixed missing Option unwrap; scanner test caught auxiliary XML
  appearing through generic XML extension, fixed before successful tests.
- Roadmap/known gaps updated, priority ticked, queue regenerated. VRT retains
  existing Add Data exclusion; expanded databases still use synchronous readers.
  These bounded gaps are recorded in KNOWN_GAPS.

## Validation (final product code)

All Cargo runs sequential, -j3, target `/home/jansolo/.cache/geoforge-target`.
From app:
- `cargo test -j3 -p rustgis-ui-egui --features unigis catalog_folder --
  --test-threads=1`: PASS3 unit +1 smoke. `/tmp/catalog-focused.log`.
- `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=1 cargo xtask ci`: EXIT1; format/workspace
  clippy PASS, engine121passed/6failed/1ignored. Same six existing plugin fixture
  failures (`nope`), documented in KNOWN_GAPS. `/tmp/catalog-app-ci.{log,exit}`.
  Adapter failures documented from prior work were not reached/rerun this turn.
- `cargo clippy -j3 -p rustgis-ui-egui --features unigis --all-targets --
  -D warnings`: PASS; `/tmp/catalog-ui-clippy.log`.
- `cargo test -j3 -p rustgis-ui-egui --features unigis -- --test-threads=1`:
  PASS45 unit +24 smoke; `/tmp/catalog-ui-tests.log`. `cargo xtask layers`: PASS.
- `cargo check -j3 -p rustgis-web --target wasm32-unknown-unknown`: PASS;
  `/tmp/catalog-web-check.log`. Final cfg-only helper guards remove web dead-code
  warnings; native bodies unchanged. App fmt check and native release build PASS.
From root: `CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target scripts/check`:
PASS (exit0), full fmt/lint/workspace tests and doc tests;
`/tmp/catalog-root-check.{log,exit}`. Staged diff check PASS; protected verifier
matches `/tmp/catalog-verifier.sha256`.

## Actual native desktop acceptance

Release app launched from `/mnt/media/Data`, isolated XDG_CONFIG_HOME
`/tmp/catalog-ui-config`. Browsed Data and Canberra imagery folder; fixture
`/tmp/catalog-desktop-fixture` showed page5/image400.JP2 and optional imagery
formats. Refresh revealed externally created image401.sid. Actual Catalog → New
→ Folder → Run completed; `/tmp/catalog-desktop-fixture/atalog_created` appeared
without Refresh (X11 helper dropped first typed character). Screenshots inspected:
`/tmp/catalog-desktop-{data,imagery,page1,page5,refreshed,created}.png`;
`/tmp/catalog-desktop-result.json`. GUI PID2456701 stopped (exit143 from SIGTERM),
relay8767 closed and own token removed. No user configuration or Data files changed.

## Preservation / next actions

Inherited dirty tracked files preserved, hashes `/tmp/catalog-inherited-hashes.json`:
agent guidance, root raster/unary code/tests, ArcGIS reference script, generated
Dashboard. All match before commit. Untracked .worktrees/, NOTES.md, scratch_te/,
research/fixtures and web-build-memory script untouched. Only this item's UI,
tests, translations and documentation staged. No resets/stashes/worktree transfers.

1. Commit staged item, then normal push with ext4 target environment so pre-push
   runs protected scripts/check. Record result here. Do not wait for acceptance.
2. Post-commit hook refreshes generated dashboard and queue; keep unrelated dirty
   files unstaged. No unfinished product edits.
3. After landing, next queue item: app.add-data-browser. Do not start it this turn.
