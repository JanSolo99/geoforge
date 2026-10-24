# Current handoff

Updated: 2026-10-24T05:56:32+00:00; last agent: Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`; starting HEAD `f71f5c1` (pushed).

## User objective / implementation

User wants Basemaps automatically loaded and OpenStreetMap on the initial map.
Implemented in `app/crates/ui-egui/src/{lib,pluginsui}.rs`:
- Default startup is a new map with OSM tiles, attribution and max zoom 19.
  Explicit sample preferences/--sample, projection/extent settings and opened
  project files retain their behavior. Startup is clean, with no undo entry.
- Pinned official Basemaps 0.1.2 downloads off the UI thread on first launch;
  existing plugin store (desktop) / preferences (web) remember it. Successful
  initialization marker respects later disabling/uninstalling. Failures retry
  next launch; existing installed/disabled plugin versions are not replaced.
- Package URL https://packages.rustgis.app/basemaps/basemaps-0.1.2.rgplugin,
  SHA256 bd9b0817158424aa8ab6bff958067a1a4948199595097eb0ed16a4219f209cb6.
  Downloaded/inspected manifest: id basemaps, WASM, edit/network permissions.
  No third-party code/assets vendored into repo.
- Regression coverage: `tests/smoke.rs` (startup defaults/preferences) and
  `pluginsui::startup_tests` (hash check, retry, caching, disabled/removal behavior).
  `examples/ui_shot.rs` gains optional `startup` script flag. Docs/roadmap updated.

## Validation / findings

PASSED on the final runtime source and screenshot example content:
- App `cargo fmt --all --check`.
- `cargo test -p rustgis-ui-egui start_up_preferences -j3`: 1 passed.
- `cargo test -p rustgis-ui-egui --lib default_plugin_download_retries_and_remembers_success -j3`: 1 passed.
- Full app `env CARGO_BUILD_JOBS=3 XDG_CACHE_HOME=/tmp/unigis-blockstats-cache cargo xtask ci`:
  exit 0, 717 passed, 0 failed, 10 ignored, 29 result groups, layers ok;
  full workspace/all-targets clippy -D warnings and formatting passed.
  Log `/tmp/unigis-basemap-app-ci.log`.
- Headless screenshot executed built `app/target/debug/examples/ui_shot` with
  RUSTGIS_PLUGINS_DIR=/tmp/unigis-basemap-shot-plugins and the cache env above;
  scripts `/tmp/unigis-basemap{,-startup}-shot.jsonl`. Inspected
  `/tmp/unigis-basemaps-startup-loaded.png` and `/tmp/unigis-basemaps-london.png`:
  plugin auto-installs without dialog, tab appears, world and street tiles render.
  Temporary store contains verified 65,048-byte basemaps.rgplugin.

False lead corrected: low-zoom OSM is unlabelled and was mistaken for the Paper
backdrop. Pixel checks show OSM water (170,211,223), land (242,239,233), distinct
from Paper (221,228,234)/(246,245,241). Generic curl requests got block PNGs;
requests using the actual app's identifying UA returned the valid world tile.
No renderer or tile-fetch changes needed. Extra `inspectTiles` diagnostics were
added to the example then fully removed, restoring its first-CI-tested content.
A redundant scoped example build (36901) and repeat CI (15449) were stopped with
SIGINT, exit 130. These are not claimed as passed; original full CI is complete
and covers final source. Avoid switching package/workspace feature graphs for
follow-up builds: it caused long redundant rebuilds on the HDD.

Browser gate PASSED: `cargo check --target wasm32-unknown-unknown -p rustgis-web -j3`
(log `/tmp/unigis-basemap-wasm.log`). Committed with this checkpoint.
No agents/coordinators launched. Builds sequential, max 3 jobs.

## Prior work / preserved edits

Aggregate CLI `f71f5c1` pushed to origin/main, verified remote SHA. Root fmt/clippy/
tests passed 173 tests, 5 ignored; root source unchanged in this task.
User authorizes frequent validated local commits; no new push requested.
Handoff push-status edit inherited and reconciled. Preserve/exclude unrelated:
tracked `tools/arcgis-bridge/__pycache__/server.cpython-312.pyc`, `.worktrees/`,
`scripts/research/unsplit/{gdbindexes_dump,index_files}.py`,
`testdata/gdb-reference/index_base.gdb/`, `index_ops_manifest.json`, and
`testdata/raster/BlockSt_dem_nod2.*`.

## Next actions

1. Finish browser check and final diff/whitespace review.
2. Commit only startup/plugin implementation, regression coverage, example and docs.
3. Report local commit and verified behavior; push only if requested.

## Claude session 2026-10-24 (pushed before the Basemaps work)

Unsplit Line (597430e), Dice (1277cb5), Alter Field/Alter Fields (3aa84cd, ecf6800), the domain
tools (97ca99a): all compared with ArcGIS state in `crates/unigis-filegdb/tests/{alter_field,
domain_ops}.rs` and `arcgis_reference.rs`. Details in `docs/ROADMAP.md`, `docs/KNOWN_GAPS.md`
(Dice small cells) and `scripts/research/unsplit/README.md`. Untracked leftovers from probing:
`testdata/gdb-reference/index_base.gdb`, `index_ops_manifest.json` (an Add/Remove Attribute Index
probe, not implemented: ArcGIS ignores UNIQUE/ASCENDING in file geodatabases, multi-field indexes
make one entry and one .atx per field), `scripts/research/unsplit/{gdbindexes_dump,index_files}.py`.
Block Statistics reference cases `blk_*` exist in `scripts/make-arcgis-reference.py` (Codex's
9bce76a implements them).

## Claude session, later 2026-10-24 (Zonal tables, cost distance)

Pushed: Zonal Histogram + Tabulate Area (0c50756; Zonal Geometry as table not done, THICKNESS rule
unknown, see KNOWN_GAPS). Cost Distance / Cost Back Link / Cost Allocation committed (`crates/unigis-raster/src/cost.rs`, `ug
cost-distance|cost-back-link|cost-allocation`, cases `cdist_*`, `cback_*`, `calloc_*`): distances and
allocation exact; Back Link ties differ on 15 cells (`cback_int` in KNOWN_RASTER_DIFFERENCES; the tie
search script `scripts/research/blockstats/cost_ties.py` is too slow to finish, make it smaller before
reuse). Not done: source cost multiplier/start cost/resistance/capacity/direction, Cost Path, Corridor,
Path Distance.
Memory note: a user report of a crash during long Python search loops; keep probes bounded
(`ulimit -v`, timeouts, background tasks) and do not run many parallel monitors.
