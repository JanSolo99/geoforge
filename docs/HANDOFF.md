# Current handoff

Updated: 2026-10-25; last agent: Claude. Workspace `/mnt/media/ArcGIS_Linux`; branch `main`.

## Objective (user: "do 1 to 5" of the speed-up plan)

Get ArcGIS tools done faster: (1) triage cheap tools in batches, (2) generate the plumbing,
(3) probe ArcGIS in bulk, (4) parallelize, (5) use CI as the gate.

## Done this session

- (5) CI: `scripts/check` + `.githooks/pre-push`; CI green again (run 40139811823). See "CI gate" below.
- (2) Plumbing: a tool needs **no clap command and no test branch** any more.
  - Raster tools: `crates/unigis-raster/src/registry.rs` (`TOOLS`: id, params, `run(&dyn Context) -> Output`).
    `ug run <id> name=value ... output=out.tif` runs them (`run_raster_tool` in `crates/unigis-cli/src/main.rs`).
  - Vector/table tools: the existing `unigis-tools` registry (`ug run`, `ug tools` lists both).
  - Aliases (a Pairwise tool is an existing command under another name): `crates/unigis-cli/src/aliases.rs`
    rewrites `ug run pairwise_buffer ...` to the `ug buffer ...` command line.
  - Reference test: `scripts/make-arcgis-reference.py` has `RUN = {arcgis tool: (registry id, [param name per
    argument])}`; the manifest case gets a `run` entry (`--annotate` adds it to existing cases without ArcGIS)
    and `arcgis_reference.rs` builds `ug run ...` from it. New tool = registry entry + one `RUN` line + cases.
  - Check `ug tools` BEFORE probing: Copy Rows, Table To Table, Join Field, Feature Envelope To Polygon,
    Generate Points Along Lines etc. already existed (snake_case ids; grep for CamelCase misses them).
- (3) Bulk probing: write many cases, run `uv run -q --with "mcp>=2" python scripts/make-arcgis-reference.py 'glob*' ...`
  once, inspect with `scripts/research/sa_batch/{summary,show}.py`, implement against the saved outputs.
  `PENDING_CASES` in `arcgis_reference.rs` skips cases of tools not implemented yet (empty now).
- (1) Tools added (all with ArcGIS reference cases, test passes): Slice, Extract by Attributes/Rectangle/Circle,
  Lookup, Sink, Pick, Flip, Mirror, Shift, Clip (raster, rectangle only), Pairwise Buffer/Dissolve/Intersect/Clip/Erase,
  Multiple Ring Buffer, Generate Points Along Lines chainage fields, Copy Rows width fix.
- (4) Parallel work is NOT launched: the coordinator rules forbid starting agents from here. The registry/alias/`RUN`
  structure means separate worktrees no longer collide in `main.rs`/`arcgis_reference.rs`; the user can run
  `./scripts/agents` per family (raster, vector overlay, geodatabase editing, conversion) in separate worktrees.

## Known differences recorded this session

- Slice: ArcGIS puts ~10 cells just below a class boundary into the class above (rule not found), allowed per case in
  `KNOWN_RASTER_DIFFERENCES`; Slice's `.vat.dbf` is not written; NATURAL_BREAKS etc. not implemented.
- Multiple Ring Buffer: dissolve ALL only; around parcels the 30 m ring has 0.05% less area in ArcGIS
  (`mrb_parcels_outside` feature 1 in `KNOWN_FEATURE_DIFFERENCES`). Pairwise Buffer draws lines and polygons with a 3.75 degree
  arc step (`--max-arc-angle`, `unigis_core::curves::set_arc_angle_step`), points as Buffer does.
- Pairwise Intersect/Erase/Dissolve list features in no fixed order: the test compares them by attributes and lower-left corner.
  Pairwise Intersect merges the pieces of one pair of features; POINT output of lines x polygons is empty.
- Raster Clip: window = cells touched by the rectangle (snapped out left/bottom, down from the top); MAINTAIN_EXTENT stretches the
  cells to the rectangle; float NoData +3.4e38; clipping to geometry not implemented. Zonal Fill not done (its output grid is odd).

## Session 2026-10-26 (Claude, "continue")

- Added Polygon Neighbors (`polygon_neighbors`, table out: src_/nbr_ field pairs, [AREA], LENGTH, NODE_COUNT; BOTH_SIDES only)
  and Count Overlapping Features (`count_overlapping_features`, single input only; rows ordered by count ascending) in
  `crates/unigis-tools/src/neighbors.rs`; cases `pn_squares*`, `cof_clip` pass `arcgis_reference`.
  ArcGIS rejects two inputs of unequal shape type (`cof_squares_clip` failed, removed).
- Subdivide Polygon (`subdivide_polygon`, NUMBER_OF_EQUAL_PARTS and EQUAL_AREAS only) implemented and passing `sbd_*`.
  Probed rule (corrects the earlier guess): ArcGIS always cuts with lines parallel to the X axis, from the bottom up, into
  equal-area strips whatever the polygon's shape or rotation; one feature per strip (multipart if split); EQUAL_AREAS gives
  ceil(area/target) strips, the top one the remainder. Other methods (STACKED_BLOCKS, split angle, width) not done.
- Tabulate Intersection (`tabulate_intersection`, in `neighbors.rs`; cases `ti_*`) implemented: zone features grouped and dissolved
  by zone fields, class features grouped by class fields but NOT dissolved (overlapping class features count twice, observed
  with no class fields); rows sorted by zone then class values, zero intersections omitted; AREA (or LENGTH for line classes)
  plus PERCENTAGE = amount / zone group area (polygon classes) or / total class length (line classes: observed, the zone's
  share rule for lines is a guess). Not done: sum_fields, xy_tolerance, point classes. ArcGIS measures on its coordinate grid:
  the test compares `ti_` tables to 2e-5 relative / 1e-3 absolute.
- Create Thiessen Polygons (`create_thiessen_polygons`, cases `th_*`, passing): Voronoi cells (half-plane clipping, O(n^2)) inside the
  points' extent grown by 4.9945% of width/height per side (observed on 3 datasets); fields: default `Id`, `Input_FID` (input FID),
  with ALL the input fields (`Id` -> `Id_1`). ArcGIS lists the polygons in an internal spatial order that is NOT reproduced (we list
  input order; the test sorts `th_` cases by attributes). Not done: the Extent environment, duplicate points, geodesic/GCS input.
- Simplify Line POINT_REMOVE (`simplify_line`, cases `sl_*`, passing): same vertices as plain Douglas-Peucker (dist > tol keeps);
  output adds InLine_FID, MaxSimpTol, MinSimpTol (= tolerance). ArcGIS also writes `<out>_Pnt.shp` (collapsed points; we do not write it: empty in
  the probes). Not done: BEND_SIMPLIFY, polygons (Simplify Polygon), barriers, error resolving (topology repair).
- Spatial Analyst single-raster Math tools (25: Abs, Negate, Square, Square Root, Exp/Exp2/Exp10, Ln/Log2/Log10, Sin..ATanH, Round Down/Up,
  Boolean Not) as `unary` in `unigis-raster/src/local.rs` + `unary_tool!` entries in the raster registry; 75 cases `um_*` (3 inputs each)
  all pass. Rules: float functions give F32 with NoData where not finite as f32; integer Abs -> unsigned same width; Negate -> signed
  (U8/S16 -> S16); Square U8 -> U16, S16 -> S32 with NoData -2147483647 (not i32::MIN); Round Down/Up always F32; Boolean Not U8 NoData 2.
  Not probed: S32/U16 inputs of Abs/Negate/Square. Next in this family: Power, Mod, Boolean And/Or/Xor, comparison tools, Bitwise.
- Two-operand Math tools (16: Power, Mod, Boolean And/Or/XOr, Equal To, Greater/Less Than (Equal), Not Equal, Bitwise And/Or/XOr/Left/Right
  Shift) as `binary` in `local.rs`, operands are rasters or constants (`in1`, `in2`); 128 cases `bm_*` (8 operand pairs each) pass.
  Rules: Power F32 (non-finite -> NoData); comparisons/Boolean U8 NoData 255 (nonzero true); Bitwise truncates to int, S32, shift count
  mod 32; Mod = C remainder (sign of dividend), float if an operand is, integer type from the operands' stored statistics when both known
  (`mod_type`, constant dividend: divisor alone sets the range), else the wider type (signed if the dividend raster or divisor raster is).
  Not probed: negative-valued stats inputs for Mod beyond Negate, U16/S32 operands, Mod type when one operand lacks statistics but is
  signed 32-bit. Next: Combinatorial And/Or/Xor, Zonal Fill, Raster To ASCII/Float, Copy Raster, Mosaic, Reclass by ASCII file.
- Raster To ASCII (`raster_to_ascii`, `conversion::to_ascii`; cases `r2a_*`, byte-for-byte, pass): header names padded to 14, NoData
  always -9999, rows end with a space, CR LF line ends, floats `%.7g` of the f32 value. `ug run` special-cases this tool to write
  text (see `run_raster_tool`). The `.prj` (Esri `Projection UTM / Zone 33 / Datum WGS84 ...` block) and `.asc.xml` sidecars are NOT
  written. Raster To Float (`.flt` + `.hdr` + `.prj`) not done. Combinatorial And/Or/XOr: see the next entry.
- Combinatorial And/Or/XOr (`combinatorial`, cases `cb_*`, 9, pass): each distinct (a,b) pair gets the next number in first-occurrence
  (row-major) order; pairs the operation calls false get 0 and use no number (And: either 0; Or: both 0; XOr: both or neither 0);
  NoData if either input is; type = `integer_output(0, max id)` (4-bit NoData 15 for small ids, S8 -128 up to 127, ...). Probed only on
  same-grid integer rasters; different grids go through `common_grid` untested.
- ATan2 (F32, both-zero -> NoData, args are y then x) and Bitwise Not (S32, -trunc(x)-1) added; cases `bm_atan2_*`, `um_bitwise_not_*` pass.
  Dashboard scanner now also reads macro-declared tools (`unary_tool!`/`binary_tool!`) and `neighbors.rs`.
  Probed but NOT done: Equal To / Greater Than / Less Than Frequency (U8; NoData 3 for 2 rasters but 16 for 3 rasters: rule unclear),
  In List. Remaining sa gaps are listed by the dashboard (`python3 scripts/dashboard/build.py`, page "missing").
- `NOTES.md` (untracked, from another agent) describes decompiling ArcGIS DLLs with Ghidra/REA. CLAUDE.md says black-box only
  (never decompile Esri binaries); I did not take part in that and did not touch it. Decide with the user before using it.

## Dashboard

`docs/dashboard.html` (open in a browser; self-contained) shows coverage of the 2028 ArcGIS Pro tools (list from
`arcpy.ListTools()` saved in `docs/dashboard/arcgis_tools.json`), per-toolbox bars, CI runs, commits, next actions and known gaps.
Rebuild: `python3 scripts/dashboard/build.py [--ci]`; `.githooks/post-commit` does it after each commit. Names are matched
loosely (lowercase alphanumerics): a tool counts as "verified" when an ArcGIS reference case exists, "implemented" when it is in
a registry, a `ug` command or an alias. First reading: 234 of 2028 (11.5%).

## Next actions

1. CI is green on the pushed registry work (run 40146927019). Keep going in batches: vector/table tools missing from `ug tools`
   (Polygon Neighbors, Tabulate Intersection, Count Overlapping Features, Create Thiessen Polygons, Integrate, Smooth/Simplify,
   Subdivide Polygon) and Spatial Analyst Zonal Fill, Combinatorial And/Or/Xor, Reclass by ASCII file, Raster To ASCII/Float,
   Copy Raster, Mosaic. Biggest toolboxes by missing count: management (435), sa (236), ia (150), 3d (145).
2. `/remote-control` is a Claude Code slash command: the user has to type it in the interactive session; it cannot be set from a run.

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

## CI gate (read before pushing)

- CI failures since 08bf5dc had two causes: a compile error in f5d6076 (`IndexEdit` alias, fixed in 5b84264) and `alter_field` comparing rows text that older GDAL (CI has 3.8) prints as `500002` where the stored snapshots have `500002.0` (test now normalizes).
- `scripts/check` runs what CI runs (fmt --check, clippy -D warnings, tests, 3 jobs). `.githooks/pre-push` calls it; enable once per clone with `git config core.hooksPath .githooks` (already set here). Skip with `git push --no-verify` only for doc-only commits.
- Tests comparing GDAL text output must not depend on GDAL version formatting; gate on `gdal_version()` or normalize.
