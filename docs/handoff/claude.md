# Lane claude handoff

Updated 2026-10-30, end of session (Claude), worktree `.worktrees/lane-claude`, branch `lane/claude`.

## User request (takes precedence over the queue; no queue tool claimed)

"Creating data like in ArcGIS Pro", in slices landed with `scripts/lane land`:
1. Create Folder / Create File Geodatabase / Create Mobile Geodatabase in the shared
   `unigis-tools` registry with ArcGIS reference cases.
2. Expose the geodatabase-schema CLI commands (Create Feature Dataset, Create Domain, Create
   Relationship Class, Add Field, subtypes, indexes, domains, ...) through the registry so the
   app's Geoprocessing pane runs them.
3. App Catalog pane: ArcGIS Pro's New menu (folder → Folder / File GDB / Mobile GDB / Shapefile;
   geodatabase → Feature Class / Feature Dataset / Table; feature dataset → Feature Class), each
   opening the tool dialog prefilled with the location; Catalog refreshes afterwards.

## Slice 1 — landed on main (93b6348)

- `crates/unigis-tools/src/workspace.rs`: `create_folder`, `create_file_gdb`, `create_mobile_gdb`
  (Report runner; messages `out_folder:` / `out_file_gdb:` / `out_gdb:` with the derived path).
  Filesystem work behind the new default feature `gdb` (optional deps unigis-filegdb, unigis-gpkg);
  the app enables it natively (`app/crates/unigis/Cargo.toml`), the web build goes without.
- `crates/unigis-gpkg/src/mobile.rs` + generated `mobile_seed.rs` (from
  `scripts/research/mobile_gdb_seed.py`, two ArcGIS-made empty .geodatabase files): writes an
  empty mobile geodatabase with our SQLite writer. Verified in ArcGIS: Describe gives
  SqliteWorkspaceFactory 3,0,0; CreateFeatureclass/CreateFeatureDataset/CreateDomain/insert work.
- Registry choice validation now reports ArcGIS's `ERROR 000800: The value is not a member of A | B.`
- App `reg.rs`: a Report without rows adds no table (Output::Layers(vec![])), messages only.
- 23 `wsc_*` references (`workspace_create_case` in the generator; fixture
  `testdata/geoproc/workspace_create`; comparator `crates/unigis-cli/tests/common/workspace_create.rs`).
- Validation: `cargo test -j3 -p unigis-cli --test arcgis_reference tools_match_arcgis_output
  -- --exact` PASS (whole manifest); negative check confirmed wsc cases are compared;
  clippy -D warnings on unigis-tools/gpkg/cli PASS; `cargo check -p rustgis-unigis` (app) PASS.
  Full `scripts/check` runs inside `scripts/lane land`.

## Findings (ArcGIS Pro 3.7)

Create Folder on an existing folder succeeds; on an existing file 999999. GDB names lose their
own extension (`g8.txt` → `g8.gdb`, `m3.sqlite` → `m3.geodatabase`); existing output 000258;
location inside a .gdb 000792; missing location 000732; `*` in a name 999999; version 9.2/9.3
writes the old 8-table layout (we refuse: KNOWN_GAPS). Multi-dot names behave oddly (not probed).
Only one ArcGIS worker can run in the Wine prefix at a time; the MCP bridge recovered after a while.

## Slice 2 — landed on main (92afc02)

- `crates/unigis-tools/src/gdb_schema.rs`: 23 schema tools in the registry (ArcGIS names/defaults
  from `GetParameterInfo`); new `registry::Kind::Path` (dataset/workspace by path; app maps it to
  `ParamKind::Path`). Paths `city.gdb[/fds]/table`; missing table 000732; Add Field same-type
  duplicate = WARNING 000012 success, other type ERROR 000012; 000310 digit-first names.
- Scenario references `gsc_build` / `gsc_edit` (`GDB_SCENARIOS`, `gdb_scenario_case` in the
  generator; comparator `crates/unigis-cli/tests/common/gdb_scenario.rs`). Test filter:
  `UNIGIS_REFERENCE_CASES=gsc_,wsc_`.
- Fixes found by the scenarios: CLI `write_geodatabase` uses the writer's ArcGIS layout (`Shape`,
  Shape_Length/Area added and computed) unless the output brings its own measure fields (copies
  keep exact values — `to_geodatabase` test); Create Feature Class/Create Table add `Id`/`Field1`
  only for .shp/.dbf outputs; filegdb subtype edits: RemoveSubtype only moves the default when it
  removes it, physical field defaults synced after SetSubtypeField/SetDefaultSubtype/RemoveSubtype.
- Validation: unigis-filegdb/-cli/-tools/-gpkg tests PASS (incl. full reference manifest and
  to_geodatabase after the copy fix); clippy -D warnings PASS; no-default-features check PASS;
  app `cargo check -p rustgis-unigis` PASS. ArcGIS read our scenario output and a buffer class.
- Lane worktree has an untracked symlink `testdata/large` → main checkout's copy (needed by
  `unigis-filegdb` oracle tests in `scripts/check`; other lanes lack it too). Never commit it.

## Slice 3 — landed on main (98033e4); user request complete

- Catalog New menus, native gdb tree (feature datasets; cache by `a00000004.gdbtable` mtime),
  `ui.gp.open` `"values"` prefill (optional layer inputs start empty, so no template from the
  active layer), path outputs written to disk by the app (`workspace::write_features/write_table`),
  "Output Name" hidden for message-only tools, reader lists empty feature datasets by item type,
  `ui_shot` `right_click` and `"last": true`. 6 strings in all 16 i18n catalogs.
- Verified by screenshots (/tmp/ugc/shots, not committed): folder/gdb/feature-dataset menus, the
  prefilled dialogs, Run creating a folder, a mobile gdb, NewFeatureClass (free name increments) and
  Transport/NewFeatureClass; ArcGIS read them (dataset SR, OBJECTID/Shape only).

## Queue: Table To Geodatabase — landed on main

- Registry `table_to_geodatabase` (`unigis-tools/src/feature_class_to_geodatabase.rs` TABLE_TOOL,
  `Context::import_tables`); CLI `feature_class_to_geodatabase::run_tables` (shared `import` with a
  table mode: dBASE/CSV/gdb tables/shapefiles → tables, CSV text 8000, feature-dataset target
  000732). 8 `ttg_*` references via the generalized `feature_import_case`.
- Validation: `UNIGIS_REFERENCE_CASES=ttg_,fctg_` reference test PASS; full `scripts/check` PASS in
  `scripts/lane land`; claim released.

## Queue: Extract Scanned Lines / Polygons — deferred (ArcScan smoothing/tracing undocumented)

- Probes and findings: `scripts/research/extract_scanned_lines/` (README, probe.py); lines in
  `docs/WORK_QUEUE_DEFERRED.txt`.

## Queue: Raster to Polyline — deferred

- Graph/ordering model established, SIMPLIFY rule not: `scripts/research/raster_to_polyline/README.md`
  (worth resuming: NO_SIMPLIFY is close to done as a model).

## Queue: CAD To Geodatabase — deferred (no native CAD reader; see WORK_QUEUE_DEFERRED)

## Queue: Extract Locations From Document — landed on main

- `unigis-tools/src/extract_locations.rs` (registry `extract_locations_document`), unit tests,
  6 `eld_*` references (testdata/geoproc/eld_*.txt; comparator
  `crates/unigis-cli/tests/common/extract_locations.rs` skips Filename/Modified/Scanned).
- Validation: `UNIGIS_REFERENCE_CASES=eld_` PASS; unit tests PASS; clippy -D warnings PASS;
  no-default-features check PASS; full `scripts/check` PASS in `lane land`. Gaps in KNOWN_GAPS.

## Queue: Extract Locations From Text — landed on main (full scripts/check PASS)

- `TEXT_TOOL` in `unigis-tools/src/extract_locations.rs` (shared engine; `Source::text`), 4 `elt_*`
  references; `UNIGIS_REFERENCE_CASES=elt_,eld_` PASS, clippy PASS.

## Queue: Create Normal Raster — landed on main (full scripts/check PASS)

- `unigis-raster` `extract::normal_raster` + registry `create_normal_raster`; 5 `cnr_*` references
  with the statistical comparator `compare_normal_rasters` (5 standard errors; ArcGIS's sample
  checked too). Probe: seeded ArcGIS runs repeat; no standard MT19937 seeding matched the uniforms.
- Validation: `UNIGIS_REFERENCE_CASES=cnr_` PASS on 6 runs with fresh seeds; unit test; clippy.

## Queue: Rescale by Function — landed on main (full scripts/check PASS)

- `unigis-raster/src/rescale.rs` + registry `rescale_by_function`; 23 `rbf_*` references
  (`UNIGIS_REFERENCE_CASES=rbf_` PASS); unit tests; clippy. Statistics presence optional for rbf_
  (ArcGIS inconsistent); research in scripts/research/rescale_by_function/README.md.

## Queue: Add Surface Information — landed on main (full scripts/check PASS)

- CLI `run_add_surface_information` + `surface_z` / `clip_segment` in `crates/unigis-cli/src/main.rs`,
  raster-registry entry (placeholder runner, like Extract Multi Values); 9 `asi_*` references PASS.
  Polygons not done (KNOWN_GAPS) — next step if resumed: SURFACE_AREA / polygon slopes.

## Queue: Zonal Fill — landed on main (full scripts/check PASS)

- `unigis-raster/src/zonal.rs::zonal_fill` + registry; 4 `zf_*` references PASS; unit test; clippy.
  Rule fitted on 30 random ArcGIS grids (scripts/research/zonal_fill/).

## Queue: Zonal Geometry — landed on main (full scripts/check PASS)

- `unigis-raster/src/zonal.rs::zonal_geometry` + registry; 9 `zg_*` references PASS; unit tests; clippy.
  Thickness = chamfer distance (1 / 1.414) to the nearest point of another zone's cell (probed).

## Queue: Zonal Geometry as Table — landed on main (b7c65f4, full scripts/check PASS)

- `zonal::zonal_geometry_table` + CLI runner (table) + registry placeholder; `zgt_*` cases un-skipped
  (run entries via `--annotate`); PASS. Two-pass centred moments were needed for last-digit match.
- The kimi lane landed its own Zonal Geometry (f6451fe) over mine (04be4a9): its `zonal.rs`
  implementation replaced mine and both reference sets (18 `zg_*`) are kept. The table tool now
  uses its `zonal_geometry` / `zonal_thickness`; all 18 + 5 cases pass on the combined code.

## Queue: Adjust Raster to Stream / Adjust Stream to Raster — deferred (2026-10-31)

- Raster to Stream: ArcGIS fails with ERROR 160327 on every input. Stream to Raster: own path
  tracing not identified. Notes: scripts/research/adjust_stream_to_raster/README.md.

## Queue: Contour List — landed on main (full scripts/check PASS)

- `contour::contour_list` (Contour now calls it), CLI `write_contours` shared with Contour, registry
  placeholder; 3 `cl_*` references and the existing contour cases PASS.

## Queue: Cut Fill — deferred (pixel type rule unknown; notes scripts/research/cut_fill/)

## Queue: Derive Continuous Flow — deferred (depression-flat routing differs from Fill+D8 in 232 cells)

## Queue: Derive Stream As Line / As Raster — deferred (depend on Derive Continuous Flow)

## Queue: Feature Preserving Smoothing — deferred (prototype in scripts/research/feature_preserving_smoothing/)

## Queue: Flow Distance — landed on main (full scripts/check PASS)

- `hydro::flow_distance` (D8) + registry; 4 `fdist_*` references PASS; MFD refused (KNOWN_GAPS).

## Queue: Geodesic Flow Direction — landed

- `hydro::geodesic_flow_direction` (WGS84 distances via `flow_direction_with`), registry
  `geodesic_flow_direction`, 4 `gfd_*` references (fixture `testdata/raster/dem_geographic.tif`).
  ArcGIS writes square cells sized from the X extent (0.0001 → 9.999999999999786e-5) and no
  statistics. `gfd_hydro` shares Flow Direction's 3 allowed flat cells. MFD/DINF refused.

## Queue: Geomorphon Landforms — landed

- `geomorphon::geomorphon_landforms`; the raster registry `Output` gained `extra` (secondary
  rasters written by `ug run` to the named output parameter). ArcGIS rules found by probing:
  sign from `max + min` angle vs threshold; diagonals limited by distance (cells: k·√2 ≤ n);
  cells strictly nearer than the skip distance are ignored; NoData/edge stops a line (no cell →
  flat); skip gives a NoData border (cells: round half down; ground: ceil, ≤ ½ cell → none);
  geographic: sphere R 6 371 000 m. `second_raster_outputs_match_arcgis` compares the second output.

## Queue: Multiscale Surface Deviation — landed

- `multiscale::{scales, multiscale_surface_deviation}`, registry `multiscale_surface_deviation`
  (second output `out_scale_raster` via `Output.extra`), 8 `msd_*` references; the second-output
  test is now `second_raster_outputs_match_arcgis` (geomorphons + scales). Findings: ArcGIS
  squares z in f32 then sums in f64 (exact for scales ≥ 4 cells; below that a few cells match
  pure f64 instead — allowed in KNOWN_RASTER_DIFFERENCES); default scales 4..13 cells;
  deviation NoData 3.4e38, scale NoData −f32::MAX; geographic transform per axis for the
  deviation raster but square for the scale raster.

## Queue: Multiscale Surface Difference — landed

- Shares `multiscale.rs` / `multiscale_surface` in the registry with Deviation (flag
  `standardize`); value z − mean, increment parameter `increment`, no nonlinearity. 6 `msf_*`
  references match exactly (no precision quirks without the variance).

## Queue: Multiscale Surface Percentile — landed

- `multiscale::multiscale_surface_percentile` (ring-incremental exact counts), registry entry
  via `multiscale_surface`; 6 `msp_*` references exact. Rule: 100·#(v < z)/n, n includes the
  cell; the scale with the largest |p − 50| (strictly greater replaces).

## Queue: Raster Calculator — landed

- `calculator.rs`: tokenizer + Python-precedence parser evaluating through `local::*`
  (registry `raster_calculator`, param `expression`). The reference generator rewrites absolute
  repo paths inside the expression to Wine paths (manifest keeps them relative). Findings:
  `/` is true division (integer rasters → F32), the Divide function keeps integer division;
  arcpy has no `Sqrt` (use SquareRoot). Intermediate types match the per-tool rules.

## Queue: Storage Capacity — landed (without zones)

- `zonal_tables::storage_capacity_table` + `ug run storage_capacity` table branch in main.rs;
  `registry::z_unit_metres` now shared (Geomorphon uses it). Area counts z ≤ level; volume =
  Σ f32(level − z) · cell area / z_unit², ZONE_CODE 1. Next for this tool: zones (raster on a
  different grid is resampled by ArcGIS; polygons) — `combine_polys.shp` is a candidate fixture.

## Queue: Stream to Feature — landed

- `hydro::stream_to_feature` + `ug run stream_to_feature` branch. Findings (all 74 arcs exact):
  sources start half a cell back unless the cell behind is a stream; vertices at direction
  changes; end at next link's first centre or half a cell past an outlet; nodes numbered in
  raster order of link starts (outlet right after its cell's start); arcs ordered by end cell
  (outlet arcs first), inflows by flow code; SIMPLIFY = Douglas-Peucker, tolerance cell/√2.

## Queue: Surface Area Ratio — landed (ARC_CHORD approximate)

- `surface::surface_area_ratio`, registry `surface_area_ratio` (+ `out_area_raster` via
  `Output.extra`). Exact: Jenness area; HORIZONTAL ratio = Σ available triangles / (n/8 · cell
  area), so edges/NoData get values while the area raster is NoData there. ARC_CHORD: docs say
  cell area / cos(local slope) on a 3×3 plane of best fit; tried Horn, LS, PCA, corner, edge,
  vector-area planes, ArcGIS Slope PLANAR/GEODESIC and UTM scale factors — none exact (median
  7e-6, max 5e-5 relative). Allowed 7041 cells in `sar_dem`. Revisit only if asked.

## Queue: Validate Flow Direction — landed

- `hydro::flow_direction_loops` + `ug run validate_flow_direction` branch (points, no output
  when valid, like ArcGIS). New fixture `testdata/raster/flowdir_loops.tif` (6 loops).
- Pitfall: generating references while the MCP bridge's ArcGIS session is also running gave a
  truncated result once (3 of 6 loops); rerunning alone was correct. Run one worker at a time.

## Pitfall: ArcGIS sidecars in testdata

Running ArcGIS on fixture rasters can leave untracked `testdata/raster/*.aux.xml` / `*.vat.dbf`
files next to the inputs; they change other tools' inputs and made `scripts/check` fail once.
After generating references, check `git status --short testdata/` and delete such untracked files.

## Next

- Next: `scripts/lane claim` for the next queue tool.
- Possible follow-ups (not requested): Catalog New for mobile geodatabases (needs a mobile reader),
  ArcGIS-style blank names instead of NewFeatureClass placeholders.
