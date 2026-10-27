# Current handoff

Updated 2026-10-27T20:06:05+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEADec66a49, frequency slice PUSHED; remote verification logged this turn. User authorizes ongoing development,
tested commits/pushes/dashboard updates while sleeping; do not ask repeated prompts.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed/pushed

Prior run: Graphic Buffer48 cases, Composite125, dashboard accuracy/cache fixes,
closed stdout, ASCII/Float imports35 cases (bec6e20). Basemaps/branding/docking landed.
This session:
-6812ba6: raster Define Projection, geographic GeoKeys table/raw/GDAL tests,5 cases.
-cd084e6: Copy Raster all bands and source NoData defaults,24 cases.
-1c830f1: large integer NoData tags, float marker normalization/stats,16 downcast cases.
All normal pre-push gates PASS. Latest gate183 Rust,5 ignored,8Python,fmt,clippy.
Logs /tmp/unigis-copy-downcast-{check,push}.log. Dashboard refreshed: CI40566379155 (projection) and40568168576 (band copy)
SUCCESS at19:41Z; downcast CI still pending.
Durable research raster_crs/copy_raster; ROADMAP/KNOWN_GAPS updated.

## Frequency slice completed/pushed ec66a49

New crates/unigis-raster/src/frequency.rs +lib/registry registrations for3 tools,
source-precision comparisons, SINGLE_BAND flatten/default and MULTI_BAND behavior.
Native caps256 comparisons, matching grid/CRS;255 with NoData refused (encoding gap).
Actual ArcGIS stores0+out-of-range256 tag for that case; GDAL mask flags1/all valid.
Do NOT claim ArcGIS uses a standard mask. Native saturation would incorrectly store255.

63 successful saved freq_* cases, generator CASES/RUN/RASTER_OUT, manifest, exact
CLI comparisons,4 tiny typed inputs testdata/raster/frequency. Research folder includes
62 preliminary observations/scripts, make_inputs.py and README with primary links.

ArcGIS MULTI_BAND repeats a single VALUE band up to max comparison bands; a multi
value raster fixes output band count. Missing comparison bands yield NoData (no
broadcast); extra comparison bands truncate. Stats empty bands count/min/mean0,
max/stddev-1. New Output.empty_band_statistics policy + stats writer helper, CLI
writer integration retain older tools' behavior. No app source changes.

CLI tests/frequency.rs: grid/CRS, empty/oversized list,255-with-NoData refusal preserves
inputs/existing output; also Rank/Popularity empty lists return000735 instead of panic.
Scoped cargo test-j3-p unigis-cli --test frequency PASS2 tests before empty-band
policy addition; initial workspace clippy PASS before final statistics policy.
FINAL scripts/check PASS exit0 /tmp/unigis-frequency-check.log:185 Rust passed,
5 ignored,0 failed;8Python,fmt,workspace/all-target clippy-Dwarnings. All63 exact
ArcGIS cases and two CLI safety tests included. fmt done, generator63/63 succeeded exit0
/tmp/unigis-frequency-generation.log. ROADMAP/KNOWN_GAPS/README updated.

## Next actions

1. Reconcile next unary math probes before implementation; all48 initial probes succeed.
   Scripts/results scripts/research/unary_types/ (currently untracked).
2. Cached-statistics isolation24 probes running session24161,
   /tmp/unigis-unary-statistics-probe.log; /tmp/unigis-unary-statistics-probe.py.
   Old DEM has cached min/max; matrix fixtures do not. Type/NoData differences must
   be explained before changing rules. Do not break old references or guess thresholds.
3. Save additional evidence, generate bounded reference cases, implement confirmed
   differences, full root gate/dashboard, commit/push. No math source edits yet.
4. Existing CopyRaster tiny tagged stats/U32 signed-export gaps remain documented.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No reset/stash/worktree transfers. Root-only edits: no local full app build.
Post-commit rebuilds dashboard HTML leaving expected dirt. No Cargo processes.
Next exploratory48 Abs/Negate/Square type probes running session76974,
/tmp/unigis-unary-types-probe.log, script /tmp/unigis-unary-types-probe.py.
Only private copies; no math source edits. Initial48 probes finished and durably saved.
Current24 statistics-isolation probes still running. Frequency normal push gate
PASS185 Rust/5ignored/8Python,fmt/clippy /tmp/unigis-frequency-push.log.
Dashboard JS node --check PASS before commit. Dashboard regenerated --ci:
282 own tools/307 verified entries. Native inventory exposes all3 new registry ids.

## Active Float64 unary checkpoint (uncommitted)

local.rs preserves F64 for Abs/Negate/Square with f64MIN NoData, without changing
integer planners. Six um64_* cases + exact comparison prefix; generator session5107
/tmp/unigis-unary-f64-generation.log. Research48 observations and native_before.txt
show22 mismatches; six are F64, others deferred until stats rules explained.
No new dependencies. ROADMAP/KNOWN_GAPS/research README updated.
Next root gate on this bounded precision slice, dashboard, commit/push.
Statistics isolation24 completed successfully; saved statistics_observations.json,
probe_statistics.py and make_statistics_inputs.py in research/unary_types.
F64 full gate session98444 /tmp/unigis-unary-f64-check.log:fmt/clippy/8Python and
six exact ArcGIS comparisons PASS; remaining workspace tests running.
Extra18 cached/tiny/F64-overflow probes session36307 /tmp/unigis-unary-extra-probe.log.

FINAL F64 scripts/check PASS exit0:185 Rust,5ignored,8Python,fmt/clippy, all6
exact reference cases. /tmp/unigis-unary-f64-check.log. After-check across48:
32 match,16 integer differences remain; research/unary_types/native_after_f64.txt.
Committing/pushing this bounded precision slice next; extra18 probes still pending.
