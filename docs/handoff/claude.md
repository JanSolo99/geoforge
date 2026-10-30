# Lane claude handoff

Updated 2026-10-30 (Claude), worktree `.worktrees/lane-claude`, branch `lane/claude`.

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

## Slice 2 — committed, landing

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

## Next

- Slice 3: Catalog New menus in `app/crates/ui-egui/src/panes.rs` (`folder_browser`), prefilled
  `ui.gp.open`; app needs output-to-disk for Create Feature Class / Create Table into a gdb.
