# Current handoff

Updated 2026-10-27T20:36:45+00:00; last agent Codex.
/mnt/media/ArcGIS_Linux; main; HEAD4fe66fc PUSHED/verified
4fe66fc017ddc6df8237628ce519116704a7a1f9. User authorizes ongoing development,
tested commits/pushes/dashboard updates while sleeping. No repeated prompts.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed/pushed this session

6812ba6 raster Define Projection/geographic GeoKeys (5 cases); cd084e6 Copy Raster
all bands/defaults (24);1c830f1 large NoData tags/float stats (16);ec66a49 frequency
family (63) and empty Rank/Popularity list guard.185 Rust/5ignored/8Python gates.
a9af367+4fe66fc Float64 math: preserve UNCACHED F64, intentionally narrow CACHED F64
toF32, matching ArcGIS.12 exact references +two cached TIFF/statistics inputs.
The first overbroad F64 push was stopped before publication; corrected4fe66fc
was the published head. No history rewrite. All normal pre-push gates passed.
Prior imports35/Composite125/GraphicBuffer48 already pushed; branding/docking/OSM done.

## Active uncached integer slice (validated; committing next)

local.rs uncached Negate:8bit->I16,16bit->I32,32bit signed wrap. Square preserves
signedness, widens8/16bit, keeps32bit, overflow->NoData. Cached integer paths remain
unchanged until planner research complete.39 umint_* references (36 typed+3 exact
boundaries), exact comparison prefix,3 source files. All48 original typed probes
now match /tmp/unigis-unary-native-after-integer.log and research/native_after_integer.txt.

FINAL scripts/check PASS exit0 /tmp/unigis-unary-integer-check.log:185 Rust passed,
5ignored,0failed;8Python,fmt,workspace/all-target clippy-Dwarnings. All39 new cases,
12 Float64 cases and prior references included. CLI build PASS30.75s. Oracle39/39
success; unsigned Square boundary warning010663 (no RAT) is expected, raster succeeds.
ROADMAP/KNOWN_GAPS/research README/dashboard refreshed. These source/docs/reference
edits UNCOMMITTED. Need JS/diff check, stage scoped paths, commit/push normally.

## Next research

scripts/research/unary_types contains48 baseline,24 stats-isolation,18 extra and3
boundary observations +scripts/recipes. Stored statistics affect integer type and
NoData decisions. Cached tiny Abs/Negate/Square follow narrow output ranges; wide
cached I16 can use I32 NoData-2147483647 instead of default-2147483648.
Do not apply uncached rules to cached inputs or break existing DEM references.

Additional30 cached32/byte probes RUNNING session6545,
/tmp/unigis-unary-cached32-probe.log, script /tmp/unigis-unary-cached32-probe.py.
Private sources /tmp/unigis_unary_cached32_*.tif. No cached planner edits yet.
Only oracle worker active; no Cargo process after final gate. No more builds until
pre-push or a new scoped source change. Root-only work; no local full app build.

## Next actions

1. Finish dashboard JS/diff checks, commit integer slice, push through mandatory gate,
   verify remote and update this handoff/dashboard. Do not repeat completed pushes.
2. Save cached32 observations/recipe, reconcile source statistics with output types.
   Implement only confirmed planner behavior in a separate bounded checkpoint.
3. Continue small tested commits/pushes. CopyRaster tiny tagged stats/U32 signed
   export differences and frequency255-with-NoData encoding gap remain documented.

## Preserve unrelated dirt

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from earlier oracle work.
No reset/stash/worktree transfers. Post-commit dashboard HTML dirt expected.
