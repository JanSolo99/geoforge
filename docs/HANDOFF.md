# Current handoff

Updated 2026-10-27T21:30:01+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEADb22e161 PUSHED/verified
b22e1614a9d1c4f03d31b599c86bedabb4e268cb. User authorizes continuous tested
commits/pushes/dashboard updates while sleeping, without repeated prompts.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed/pushed this session

6812ba6 Define Projection/geographic GeoKeys5 cases;cd084e6 Copy Raster all bands24;
1c830f1 large NoData/rounded float stats16;ec66a49 frequency family63;4fe66fc correct
cached/uncached F64 math12 (a9af367 is historical parent, never sole published head);
c2ed0cf uncached integer widths/wrapping/overflow39; b22e161 opt-in GP zero encoding
of out-of-range integer NoData, frequency255-with-NoData3 cases now works.
Latest full/push gates186 Rust passed,5ignored,8Python,fmt/clippy. All normal hooks.
CI through c2ed0cf SUCCESS at last fetch; b22e161 pending. Dashboard282 own tools/
307 verified entries. Branding/docking/OSM and prior imports35/Composite125/Graphic
Buffer48 already landed. Do not revisit stale old chat requests.

## Active cached integer planner (UNCOMMITTED)

local.rs cached_integer_unary uses conservative ranges: Abs/Square min0, Negate
[-max,-min]; packed/8/16-bit and range-aware32-bit types/NoData. Signed upper bound
is inclusive; unsigned zero-min ranges promote at the signed top. Fallback S32 can
carry NoData -f64MAX; new GP writer flag produces physical0 and logical statistics.
U32 Negate wraps; signed overflow becomes missing. Invalid stats leave old path.

27 snapshots TIFF+ArcGIS CalculateStatistics aux under testdata/raster/unary_cached,
cached_inputs.json +make_cached_inputs.py preserve/recreate pixels with real sidecars.
81 exact umcache_* references generated successfully. Initial native72 comparison
71 matched; i32_ndmin Negate endpoint failed, corrected. Rebuild/72 comparison PASS.
Added symmetric signed32 endpoint3 cases from independent successful probe.

Full gate RUNNING session71766 /tmp/unigis-unary-cached-check.log:
fmt/clippy/8Python PASS; generic ArcGIS comparison PASS including all81 new cases,
prior uncached39/F64_12/frequency66 and all existing references. Remaining workspace
format tests still running. Do not claim whole gate passed until exit0.
No other builds. Docs ROADMAP/KNOWN_GAPS/README updated; dashboard --ci refresh
session99487 /tmp/unigis-unary-cached-dashboard.log. Next JS/diff checks and commit.

## Research / next follow-up

scripts/research/unary_types includes baseline48,stats24,extra18,boundary3,cached32_30,
cached16_6,range12,symmetric3 and scripts/recipes. cached_native_compare.txt72 matches.
Range controls prove conservative min0 for Abs/Square even if actual zero absent.
Source remains physically F64 after statistics; cached F32 narrowing is tool behavior.

Copy Raster statistics isolation6 running session41713,
/tmp/unigis-copy-statistics-probe.log, script /tmp/unigis-copy-statistics-probe.py.
Private pixel-identical large/small/multiband datasets compare with/without cached
statistics, to resolve tiny tagged-input stats without guessing a size threshold.
No Copy Raster source edits. nodata_encoding/probe_copy.py has6 separate argument
observations; override/remapping differences remain documented. These are not fixed
by the writer policy. Save new observations/recipe before choosing next patch.

## Remote management of the coordinator (2026-10-28)

`./scripts/agents --daemon` runs the coordinator headless (file inbox, keepalive "continue the HANDOFF" with back-off,
Claude/Codex fallback as before); `./scripts/agents-remote status|log|follow|submit|cmd|pause|resume|stop|web` steers it;
`tools/agent-coordinator/REMOTE.md` has the Tailscale steps (sudo tailscaled + `tailscale up --ssh`, user systemd unit
`unigis-agents.service`, `tailscale serve` for the status page). Tailscale is installed here but logged out: the user must run
`sudo tailscale up --ssh` (interactive login) and stop the interactive coordinator before starting the service.
The web page + dashboard now run as the user service `unigis-web` on the tailnet address: http://100.99.35.95:8765/ and /dashboard (machine `cachyos-x8664-howard`; tailnet-only, bound to the tailscale IP; `tailscale serve` needs sudo and was not used).
`agents-remote chat` is a prompt attached to the daemon (so a terminal can stay open while it runs headless); `agents-remote web` also serves docs/dashboard.html at /dashboard (unit `unigis-web.service`, publish with `tailscale serve --bg 8765`).
Tested offline (`test_daemon_runs_inbox_tasks_commands_pause_and_keepalive`); not yet run against real CLIs.

- Surface Parameters (probed 2026-10-28, NOT implemented): SLOPE/ASPECT/MEAN, PROFILE, TANGENTIAL, GAUSSIAN curvature with the default quadratic 3x3 neighborhood are within ~0.1% (slope) and ~0.5% (curvature) of Wood's least-squares quadratic fit (G=(z3+z6+z9-z1-z4-z7)/6c etc.) but not equal, and differ from both `Slope` and Evans-Young; first row/column are NoData; PLAN_CURVATURE is refused ("not within the defined domain"; ArcGIS calls it PLAN... use the keyword from `arcpy.Usage`). The exact weighting is unknown.

- Apportion Polygon (`apportion_polygon`, AREA + SUM only; `apo_*` pass via MEMORY_OUT): target polygons + each apportioned field as Double = sum of value x (overlap / source area); integer source fields are truncated. MEAN (and other statistics) NOT done: ArcGIS gave 0.588 / 0.833 for ZONE MEAN on clip_areas whose rule I could not match (not an area-weighted mean). Points/Length and estimation features not done.

- Euclidean Back Direction (`euclidean_back_direction`, `ebd_*` pass): the Euclidean Direction azimuth unrounded, F32, NoData 3.4e38, no statistics sidecar. Distance Accumulation / Distance Allocation (sa) not probed yet.

- Distance Accumulation / Distance Allocation without barrier/surface/cost rasters (`distance_accumulation`, `distance_allocation`, `dacc_*`, `dalloc_*` pass): same values as Euclidean Distance/Allocation; no statistics sidecar; allocation is Byte (16-bit beyond 255) with NoData 0 when no cell is 0, no NoData otherwise. With a cost raster ArcGIS differs from Cost Distance (max 130 on cost_f32): NOT done.
- WARNING (cost me a test run): never run `gdalinfo -stats` on files in testdata/raster or elsewhere in testdata: it writes `.aux.xml` statistics beside the input and changes how the tools type their outputs (inputs with stats take the "known range" path). Never `git add -A` blindly after probing: stage with explicit paths (crates scripts docs and the new testdata/arcgis-reference/<case>* files). Also ArcGIS probes can leave auto-named rasters (`EucDist_cost_sr1.tif` ...) in testdata/raster: delete untracked files there before committing.

- Focal Flow (`focal_flow`, `ff_*` pass): bits E1 SE2 S4 SW8 W16 NW32 N64 NE128 set for each neighbor whose elevation exceeds the cell's by MORE than the threshold (z units, default 0; neighbors outside or NoData give no bit); U16 NoData 65535 on NoData cells.

- Probed 2026-10-28 and left: Generate Rectangles Along Lines (fields PageNumber, GroupId = line FID as text, SeqId, Previous, Next, Angle; rectangles `length x width` along each line, first one starting at the line start, 'Upper left' etc. changes the anchor corner; roads give 30 rectangles but the sequence for the multi-vertex lines (restarting SeqId 1..8 then 1..3 for line '2') is unexplained), Raster To Polyline (ArcInfo-style arcs with from_node/to_node; numbering order unknown), Remove Overlap Multiple (center line = medial axis), Split By Attributes (writes tables as null-shape shapefiles).

- Probed 2026-10-28 and left: Line Statistics (LENGTH value = 3.0x the length inside the search circle in one cell, 1.38x in another: formula unknown; other statistics also unexplained). Slice NATURAL_BREAKS: ArcGIS breaks on dem.tif 5 classes lie between 209.763|209.772, 231.463|231.467, 253.554|253.563, 279.088|279.113, exact Jenks on all 18000 cells gives 209.69|209.72, 231.32|231.33, 253.40|253.42, 279.01|279.04 (0.05-0.15 lower), so ArcGIS runs Jenks on a binned or sampled version (histogram Jenks with 128..2000 bins of the value range was tried: breaks 209.5-209.7, 231.3-231.9, 253.3-254.3, 278.9-280.0, none matches 209.77/231.47/253.56/279.10); STANDARD_DEVIATION and GEOMETRICAL_INTERVAL are refused by `arcpy.sa.Slice` here (domain error), only EQUAL_INTERVAL/EQUAL_AREA/NATURAL_BREAKS exist in this build.

- Snap Pour Point (`snap_pour_point`, `spp_*` pass; new test points `testdata/geoproc/pour_pts.shp`): each point -> highest-accumulation cell within the snap distance (cell centres, point at its own cell centre; ties nearest); value = FID (0-based) or the chosen field, typed by the 8-bit integer rules (U8/15 for 0..5, S8 for 10..60); statistics written.

- Probed 2026-10-28 and left: Stream To Feature (one arc per stream link; geometry is derivable: the line starts half a cell UPSTREAM of the link's first cell centre along its flow direction (an edge midpoint or a corner), runs through the cell centres where the direction changes and ends at the centre of the first cell of the downstream link; from_node = the link's own value, to_node = the downstream link's value; the ORDER of arcs (arcid) is by to_node with ties and the late part of the list in an ArcInfo scan order that I could not reproduce; outlets' to_node unknown). Implementing the geometry with a sort that matches only partly would fail the reference test, so it is not done.

- Sample (`sample`, `smp_*` pass; rasters `a;b`, locations = points, NEAREST/BILINEAR, unique_id_field; handled by `run_sample` in the CLI because it writes a table): columns = location layer name (FID or the id field), X, Y, one per raster named by file stem (float rasters get `_Band_1`, all cut to 10 characters); F32 values keep 6 significant digits (Single field F 13.11), F64 and integers exact. Point on a cell edge -> the cell to its left / above. Outside raster: 0 if the raster has no NoData, else the NoData value (float) / -9999 (integer). Bilinear: outside neighbours count as 0 for rasters without NoData; with NoData, missing and NoData neighbours are dropped and weights renormalised. Not done: multiband rasters, buffer_distance/statistics options, ALL_SLICES.

- Extract Multi Values To Points (`extract_multi_values_to_points`, `emv_*` pass; `run_extract_multi_values` + shared `sample_value` in the CLI): adds one field per raster (name given or file stem, 10 chars; Single for F32, Double for F64, Integer for integer rasters unless BILINEAR, which makes them Single) with the Sample values; in the reference test its interpolated singles are compared to 3e-6 relative because ArcGIS's single-precision arithmetic differs in the sixth digit (feature 9 of the hydro case, 86.7259 vs 86.726).

- Point Statistics (`point_statistics`, `ps_*` pass; closes the old "Point Statistics offset" gap): grid origin = points' extent minus (w+h)/798 in both x and y (fits 4 point sets), columns/rows = round((w + 2 off)/cell), (h + 2 off)/cell); cell value = MEAN/MAXIMUM/MINIMUM/RANGE/STD(population)/SUM of the points whose distance to the cell centre is inside the Circle/Rectangle/Annulus (MAP or CELL units); F32, NoData -FLT_MAX where no point. Default cell size not probed.

## Next actions

1. Keep going in probe -> implement -> reference-test batches (see "Session 2026-10-26" entries for what is done and the gaps).
   Cheap families left: Spatial Analyst (Sample column naming, Extract Multi Values To Points, Surface Parameters, frequency tools),
   Edit toolbox (Extend/Trim Line, Snap, Erase Point: need selections), vector (Apportion Polygon, Remove Overlap Multiple,
   Aggregate Points/Polygons, Calculate Polygon Main Angle (rule unknown: not min-area rectangle, not Feret, not PCA)),
   conversion (Table To dBASE, Feature Class To Shapefile: workspace outputs; ArcGIS makes shapefiles with null shapes from tables).
2. Geodesic length/area (needs unigis-proj in unigis-tools) unlocks ADD_SHAPE_SUM of Summarize Within/Nearby with line/polygon inputs.
3. `MEMORY_OUT` in the reference script runs geodatabase-only tools through memory and copies to a shapefile.
4. `/remote-control` is a Claude Code slash command: the user has to type it in the interactive session.

## Preserve unrelated dirt

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No reset/stash/worktree transfers. No local full app CI for root-only changes.
Post-commit regenerates dashboard HTML leaving expected dirt. Never bypass push gate.

FINAL cached planner scripts/check PASS exit0:186 Rust,5ignored,8Python,fmt/clippy,
all81 new exact references plus previous corpora. /tmp/unigis-unary-cached-check.log.
Dashboard --ci regenerated. Next scoped cached commit/push with standard gate.
Copy statistics control probe session20124 remains separate, no copy source edits.
