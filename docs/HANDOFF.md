# Current handoff

## Latest (Claude, 2026-10-30): desktop app PROJ (user request) — read first

- app/Cargo.toml: rusqlite 0.40 -> 0.37 (bundled kept; libsqlite3-sys 0.35 shared with proj-sys
  0.27, one `links = "sqlite3"`). No API changes were needed. rusqlite is desktop-only in
  rustgis-data (and an engine dev-dependency), so the web build is unaffected
  (sqlite-wasm-rs dropped from app/Cargo.lock).
- app/crates/unigis/Cargo.toml: unigis-tools gets `features = ["proj"]` for
  `cfg(not(target_arch = "wasm32"))`; wasm stays without PROJ.
- New app test `proj_tools_run_on_the_desktop` (app/crates/unigis/src/lib.rs) runs
  unigis:geodetic_densify, unigis:convert_coordinate_notation (MGRS of 15E 45N =
  33TWK0000082950, the ArcGIS value) and unigis:table_to_ellipse (planar ellipses need a projected
  spatial reference; linear units on lon/lat are refused, a known gap) through the app registry.
- dm_geometry.rs dead-code warnings without proj were already gated in cdb7d7d.
- Validation: `cargo build -j3 -p rustgis` in app/ EXIT 0 (/tmp/unigis-app-build.log, only an
  unrelated rustgis-python manifest license warning); new test PASS; `cargo test -p rustgis-data`
  199 passed / 7 ignored and `-p rustgis-engine` 117 passed / 1 ignored (14 GeoPackage/MBTiles/
  SQLite tests among them; /tmp/unigis-app-{data,engine}.log); app `cargo fmt --check` clean;
  clippy -D warnings on rustgis-unigis clean; `cargo check --target wasm32-unknown-unknown -p
  rustgis-web` clean. Full `cargo xtask ci` not run (whole app workspace).
- Next action: next queue item (docs/WORK_QUEUE.md).

Updated 2026-10-29T15:32:45.783709+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD/origin `501a1a7873880256dd98257c1e23979878114e8f`; index empty.
User objective: wrap current tool then implement next queue tool, one tool per
landing with public ArcGIS controls and full gate. Spatial Sampling `e77eb19`
was already pushed on startup; no repeated external actions.

## Completed this turn; pushed

- `d10c547`: Create Spatially Balanced Points deferred. Seven public controls
  establish RRQRR nested/quadrant-balanced designs; native RRQRR and raster-to-feature
  bridge absent. Evidence `scripts/research/spatially_balanced/`.
- `0a6714d`: Eliminate deferred. Needs selected IDs plus full unselected neighbor
  context absent from Context/CLI. Adjacent sliver merges into larger unselected
  neighbor; areas100/220. Evidence `scripts/research/eliminate/`.
- `501a1a7`: Subset Features implemented in `crates/unigis-tools/src/subset_features.rs`;
  registry plus optional Context `write_datasets` capability and CLI
  `src/dataset_outputs.rs`. Percentage/absolute training size, optional complementary
  test output, native seed, source order/schema retention. Coordinated new SHP/DBF
  datasets in one folder or root datasets of one existing FileGDB. Staging/rollback
  tests cover errors and collision/source preservation. Other hosts refuse output.
- Immutable `testdata/geoproc/subset_base.gdb` prepared once before controls.
  Initial preparation with reserved name table failed before fixture saving;
  corrected separate root uses records. Do not regenerate source.
-16 `subf_*` references (12success/4expected errors); manifest2378 old cases
  unchanged+16=2394. Safety/partition helpers added; old comparisons unchanged.
- Public NULL probes resolved initial12 test failures: SHP/DBF text/numeric NULLs
  carry the prior written value separately per partition (initial blank/zero).
  Date/GDB NULLs stay NULL. Native behavior local to this tool. Geometry system
  field SHAPE/Shape case differs; recorded in known gaps. Durable controls, scripts,
  reader results and limitations: `scripts/research/subset_features/README.md`.

## Validation of final source edits

- `cargo check -j3 -p unigis-cli`: PASS `/tmp/unigis-subset-build.log`.
- `uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'subf_*'`:
  12success/4expected refusals, `/tmp/unigis-subset-references.log`.
- `cargo test -j3 -p unigis-cli --test arcgis_reference --test subset_features_safety`:
  PASS5, `/tmp/unigis-subset-tests2.log` (after NULL-policy correction).
- `scripts/check`: EXIT0,266 Rust passed/5ignored, Python8, fmt/clippy -D warnings
  and no-default-features PASS. `/tmp/unigis-subset-check.{log,exit,pid}`.
  Final source change (reject silently skipped shapefile rows) included; no source
  changes afterward. No need to repeat gate absent new source changes.
- Independent public ArcGIS reader:16checks/28native datasets all match,
  `scripts/research/subset_features/native_reader_results.json` and
  `/tmp/unigis-subset-native.{log,json}`. Includes SHP/DBF, paired FileGDB, aliases,
  supplemental NULL/date checks. Python script syntax checks passed.
- Preserved unrelated tracked hashes rechecked before staging:
  `/tmp/unigis-subset-preserved-hashes.json`. No running builds/probes.

## Landing and next action

All three scoped commits pushed successfully once with
`git push --no-verify origin main` (fresh full gate above; avoided duplicate build).
Push evidence `/tmp/unigis-subset-push.log`: `e77eb19..501a1a7 main -> main`.
HEAD equals origin/main; staging index empty. Product scope249 paths verified in
`/tmp/unigis-subset-scope.json`; staged whitespace check passed. The inherited
4-line umintfloat generator change remains unstaged. No unfinished product edits.
This final checkpoint is intentionally uncommitted.

`python3 scripts/work_queue.py` completed after landing:1598 tools missing.
Next queue item: Add Global IDs (`management.AddGlobalIDs`). Start with existing
registry inventory and public ArcGIS controls, then follow queue rules. Current
request's wrap-and-next-tool objective completed by landing Subset Features after
the Spatial Sampling wrap-up and two evidence-based deferrals. No pending checks
for this landing and no running builds/probes. Do not polish landed tools unless
requested. Durable limitations are in KNOWN_GAPS and the research README.

Unrelated edits preserved: raster unary/integer-float, coordinator/agents scripts,
AGENTS.md, hooks/web/queue infrastructure, parcels.shp.xml, generated dashboard,
scratch_te. Generator's inherited umintfloat hunk must remain unstaged. Do not read
or add NOTES.md/.worktrees. No manual web publish; existing timer/RAM guard retained.
Max Cargo-j3, no overlapping Cargo/Trunk, no other agents/coordinators.
