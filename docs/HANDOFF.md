# Current handoff

Updated 2026-10-27T19:47:12+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEAD1c830f1 PUSHED/verified
1c830f1c53e8b60b6e066464d633dce30dd4a30a. User authorizes ongoing development,
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

## Active frequency slice (VALIDATED; committing next)

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

1. Commit and push validated frequency slice through normal gate; verify remote.
2. Refresh dashboard --ci and validate JS; stage only frequency/docs/reference paths.
3. Commit/push through mandatory gate, verify remote, record exact tests and CI state.
4. Continue bounded slices. Existing CopyRaster tiny tagged stats/U32 signed-export
   differences remain documented; no guessed size thresholds. Consider these later.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No reset/stash/worktree transfers. Root-only edits: no local full app build.
Post-commit rebuilds dashboard HTML leaving expected dirt. No Cargo processes.
Next exploratory48 Abs/Negate/Square type probes running session76974,
/tmp/unigis-unary-types-probe.log, script /tmp/unigis-unary-types-probe.py.
Only private copies; no math source edits. Dashboard regenerated --ci:
282 own tools/307 verified entries. Native inventory exposes all3 new registry ids.
