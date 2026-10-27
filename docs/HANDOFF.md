# Current handoff

Updated 2026-10-27T20:20:16+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEADa9af367 LOCAL ONLY. Origin last verified
at ec66a49c2e3e1d35a77908714860bfe3f43c733a. User authorizes ongoing tested commits,
pushes/dashboard updates while sleeping. Do not ask repeated continue prompts.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed/pushed this session

-6812ba6 raster Define Projection and geographic GeoKeys,5 cases/raw-key/GDAL guards.
-cd084e6 Copy Raster all bands/default NoData,24 cases.
-1c830f1 large integer NoData metadata and rounded float stats,16 cases.
-ec66a49 Equal/Greater/Less Than Frequency,63 cases, both band modes, precision/type
 boundaries/empty-band stats; safe refusals and Rank/Popularity empty-list panic fix.
Latest full gates185 Rust passed,5 ignored,8Python,fmt/clippy. All normal push gates pass.
CI at last refresh: first3 commits SUCCESS, frequency still pending. Dashboard282
own tools/307 verified entries. Research raster_crs/copy_raster/frequency durable.
Prior session imports35 cases, Composite125, Graphic Buffer48 and dashboard fixes
already pushed. Basemaps/branding/docking landed earlier; old chat requests stale.

## Active Float64 correction — DO NOT push a9af367 alone

Initial a9af367 preserves F64 Abs/Negate/Square unconditionally; full gate passed185
Rust/six exact um64_* cases but later oracle exposed a cached-input regression.
ArcGIS preserves UNCACHED F64, intentionally narrows CACHED F64 toF32 (operation
at source precision before output cast). Initial push stopped only its own process
group2857103 during pre-push gate, before publication. Verified origin remainsec66a49.
No reset/amend/stash. Keep local checkpoint; commit correction, then push latest HEAD.

Current UNCOMMITTED local.rs guard requires statistics.is_none(). Six new cached
um64_* cases and two source TIFF+ArcGIS aux statistics sidecars under
 testdata/raster/unary_types. Existing six uncached cases retained, exact comparisons
for all12. Cached oracle generation6/6 succeeded /tmp/unigis-unary-f64-cached-generation.log.
ROADMAP/KNOWN_GAPS/research README corrected to distinguish both paths.

Corrected full gate RUNNING session40662 /tmp/unigis-unary-f64-corrected-check.log.
Final fmt/clippy/8Python PASS; Rust tests compiling at latest checkpoint. Do not
claim whole corrected gate passed until exit0 and all12 native cases succeed.
Only one Cargo gate active. Dashboard --ci refreshed (log corrected-dashboard).

## Math research / next slice

scripts/research/unary_types has48 baseline probes,24 stats-isolation probes and18
extra cached/tiny/F64-overflow probes, scripts/recipes and native_before/after_f64.
After six F64 fixes:32/48 match,16 integer mismatches remain (before correction,
uncached inputs only). Integer source has NOT been edited.
Cached stats demonstrably alter integer type/NoData; do not guess or break existing
cached DEM references. Examples: narrow I8 Negate I16 uncached/I8 cached; narrow I16
Square I32 uncached/U16 cached; wide I16 Square I32 both but ND MIN vs MIN+1.
Raw observations explain exact values. Other cached integer promotion gaps documented.

Boundary3 probes succeeded; saved boundary_observations.json and recipe/script.
Original session13683 /tmp/unigis-unary-boundary-probe.log,
script /tmp/unigis-unary-boundary-probe.py. Private GDAL-created source fixtures
under /tmp/unigis_unary_boundary_*.tif; only file IO, ArcGIS remains GP oracle.

## Next actions

1. Finish corrected full gate; inspect all12 native references. Fix failures if any.
2. Validate dashboard JS/diff, commit correction +cached source/reference files,
   push latest HEAD through gate and verify remote. Never publish a9af367 alone.
3. Continue bounded uncached integer math slice: save boundary evidence, create
   reference cases before modifying type/overflow rules. Keep cached planner separate
   until fully understood. Current16 differences: I16 Negate,32bit Negate wrapping,
   signed Square widths/overflow. Abs integers already match baseline.
4. Update dashboard/roadmap/gaps and frequent tested commits/pushes throughout.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No worktree transfers. Root-only changes: no local full app build.
Post-commit rebuilds dashboard HTML leaving expected dirt. Do not bypass push gate.

Corrected generic ArcGIS test PASS including all12 Float64 cases; remaining workspace
checks running. Draft NEXT uncached integer change prepared outside repo at
/tmp/unigis-unary-uncached-edit.py; NOT applied. Wait until corrected F64 push completes.

FINAL corrected scripts/check PASS exit0:185 Rust,5ignored,8Python,fmt/clippy,
all12 exact cached/uncached references. /tmp/unigis-unary-f64-corrected-check.log.
Committing corrective follow-up then pushing newest HEAD; a9af367 will be included
only as historical parent, never the published main head. No history rewrites.
