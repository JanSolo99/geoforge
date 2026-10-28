# Current handoff

Updated 2026-10-28T15:47:00.438445+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEADc5064bc PUSHED with normal hook and
verified remote SHA (latest tool output). User authorizes continuous development,
frequent tested commits/pushes/dashboard updates. Work alone: no agents/CLIs/
coordinators. Cargo-j3, one build at a time. Preserve NOTES.md (do not read/add),
.worktrees/. Dashboard inherited dirt is generated automatically.

## Completed current resumed session

c5064bc: shared band/domain/quote-aware cache parsing; cached extrema required in
EVERY band. Incomplete caches recompute all CopyRaster bands and keep unaryF64;
complete caches preserve and narrowF32. Floating Abs/Negate/Square now retain ALL
bands instead of silently dropping band2.20 new ArcGIS refs,11 frozen snapshots,
native planning regression, reproduction recipe verified equivalent with GDAL.
Full scripts/check AND normal push hook PASS193Rust/5ignored/8Python/fmt/clippy.
Logs /tmp/unigis-multicache-{check,push}.log; dashboard--ci/JS/Python/diff PASS.
Research scripts/research/copy_raster/multicache, formats/raster-statistics.md.
All prior Codex/Claude changes are committed ancestors (9b4dcaf and recent Diff/
Over, ComputeChangeRaster, BinaryMask, four tessellation shapes). Current full gate
validated them; ROADMAP/KNOWN_GAPS now record their coverage and limits.

## Active transverse hexagon slice (new uncommitted edits)

Primary8 controls /tmp/unigis-tess-transverse-probe.log EXIT0. Six projected-meter
frames a/b/c/d/tiny/thin give20/20/36/21/1/5cells. Side=sqrt(area/(1.5sqrt3));
width=sqrt3*side, row step1.5side; odd rows shift half-width. First centre at lower
left extent, clockwise ring from top vertex, same GRID_ID and row-major ordering.
Columns ceil((width_extent+width/2)/width), rows ceil((height_extent+side)/(1.5side)).
Added enum/constructor branch and registry choice; six tess_trhex_* generator cases.
No app edits/API field changes. Native build PASS /tmp/unigis-tess-transverse-build.log. Six goldens generated
successfully; all six exact ordered-ring/ID/count comparisons PASS
/tmp/unigis-tess-transverse-after.log. Full scripts/check PASS193Rust/5ignored/8Python/fmt/clippy, exit0;
/tmp/unigis-tess-transverse-check.log. Dashboard--ci/Python/diff PASS; JS check
immediately before commit. Preparing commit, normal push still pending.

## Next actions

1. Generate six tess_trhex_* goldens; inspect exact geometries/IDs/order, build and
   full scripts/check-j3. Update ROADMAP/KNOWN_GAPS and research recipe/observations.
2. Dashboard--ci +JS/Python/diff checks, explicit-path commit, normal push, remote
   verification. Do not overlap builds; checkpoint during work and before stopping.
3. Next separate slice: tessellation area units.15 private controls completed:
   /tmp/unigis-tess-units-probe.log. Eight rarer units EXIT0 session47818,
   /tmp/unigis-tess-units-extra-probe.log. No area-unit source edits yet.

## Area-unit findings for next slice

Current size parser ignores unit suffix (wrong hectares/km sizes). Projected meters:
sqm/km/hectare/acre/feet/yards/miles/cm/ares equivalents produce same20cells/10000m2.
Projected US feet2272:10000sqm=>4cells/107638.673611ft2; bare10000=>20cells/10000ft2.
SquareFeet vs SquareFeetUS differ (1e8area=>side9999.98 vs10000USft). No SR+10000sqm
=>20cells, unknownSR: default coordinate unit is meter if SR absent. Reuse existing
registry::meters_per_unit with squared factor; bare sizes remain coordinate squared.
Private feet_template.shp available for future fixtures. No mutation of repo inputs.

## Other durable limits

H3 tessellation missing. SAR tools refuse ordinary rasters003514 (no radar fixtures).
SurfaceParameters not a fixed linear3x3 filter; prior regression residual2e-5,
not implemented. ComputeChangeRaster categorical/spectral/multiband and BinaryMask
flood-fill/expansion/min-region/BACKGROUND_NODATA unsupported. Float unary other
operators/integer multiband and malformed/nonfinite caches unverified. See ROADMAP,
KNOWN_GAPS and committed research. Latest recovery logs read from coordinator
.git/agent-coordinator/20261028T150119-562239a4 (Claude usage_limit); no concurrent
CLI/build/push remains from it.

Current full scripts/check48827 COMPLETE PASS /tmp/unigis-tess-transverse-check.log;
193Rust/5ignored/8Python/fmt/clippy. Research recipe/results and roadmap/
known gaps updated. Do not overlap Cargo. Area-unit extra8 controls all succeeded:
mm/dm/inch/nautical-mile and US survey yard/acre/mile/inch equivalents.23 private
area controls total; no area conversion product edit yet.

Area correction plan saved /tmp/unigis-tess-area-plan.md; no product edit. Current
constructor square-count +1 can overflow for tiny areas/infinite extent spans;
plan safety guard alongside area parsing (native excessive-grid refusal, no claim
of exact ArcGIS error codes for resource-exhaustion inputs). Area control logs23
successful. Current transverse full gate48827 continues.

Milestone: source reviewed; full gate exited0; dashboard--ci exited0. Preparing
explicit-path transverse commit. No builds/oracle workers running. Next work area
parsing and extreme-size guards only after this push hook finishes.
