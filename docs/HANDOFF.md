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

## Next actions

1. Finish final cached full gate; record exact totals and JS/diff results.
2. Stage only cached planner/docs/snapshots/reference paths, commit/push normally,
   verify remote and refresh this handoff. No repeated completed external actions.
3. Inspect/save Copy Raster6 results, isolate metadata policy, add reference cases
   before implementing. Continue bounded tested commits/pushes/dashboard updates.

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
