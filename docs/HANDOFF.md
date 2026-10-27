# Current handoff

Updated 2026-10-27T16:17:37+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; branch main; HEAD a646c78, PUSHED/verified
 a646c78aae9bfd94ac13e049370fc150bfe8caf3. User authorizes ongoing commits/pushes.
Do not launch another agent, CLI session or coordinator.

## Objective

Keep developing, testing, committing, pushing and updating dashboard while user sleeps,
without repeated continue prompts, until unable. Prior Claude usage-limit run recovered
from .git/agent-coordinator/20261027T101238-693a3583/. Basemaps/branding/docking already
landed; old chat requests are stale. Current work is root native engine/CLI breadth.

## Completed and pushed this Codex run

- d66383c / cdc23aa / c3abd4a: finish inherited Graphic Buffer, caps/joins/holes,
  miter-limit bevel fallback, erosion/zero polygons, numeric field replacement,
  numeric/text distances, units and safety guards. 48 ArcGIS cases +3 refusal tests.
- ebc2d2d / 735d4f3: accurate dashboard reference coverage; cached CI survives
  offline hooks, fetch timestamp/latest gaps, 8 Python tests.
- d29a79a / eeffa40 / a646c78: Composite Bands (125 cases), true typed U32/F64
  fixtures, all-band exact pixel/statistics comparisons, type promotion, NoData
  tagged-input ranking, signed U32 sentinel export, per-source and zero padding.
  Initial AAIGrid fixture inference narrowed values; corrected typed arrays and
  read-back verification now cover all23 fixtures. Detailed research/composite/README.md.
- bab1f21: ug tools closed stdout no longer panics; deterministic regression test.
  Normal inventory retains every registered tool; pipefail ug tools | head succeeds.
CI through d29a79a SUCCESS; refresh CI status for later pushes. a646c78 push gate
passed179 Rust tests,5 ignored,8 Python,fmt/clippy; /tmp/unigis-composite-outside-push.log.

## Current slice: ASCII To Raster / Float To Raster (validated, committing next)

New crates/unigis-raster/src/grid_import.rs, lib/registry registration, CLI exact
import pixel comparison and input-preservation test tests/grid_import.rs. 35 saved
cases (31 successes,4 expected errors), generator CASES/RUN/RASTER_OUT, manifest,
ascimp_* / fltimp_* results and raw input files under testdata/raster/grid_import.
Research/reproducer: scripts/research/grid_import/README.md, make_inputs.py, probe*.py,
observations.json. No new dependencies or GDAL runtime.

ArcGIS observations implemented: INTEGER S32 truncates fractions, FLOAT/binary F32;
system NoData S32 -2147483647 / F32 min. ASCII compares raw doubles before conversion,
binary normalizes header NoData to F32. Corner/center origins, endian choices,
default ND-9999/defaultLSB. Short data010067; missing float header010062; extras
ignored. Projection sidecars deliberately ignored (ArcGIS legacy behavior).

Important oracle side-effect: bad FloatToRaster probe deleted its INPUT header.
Generator now isolates both legacy import tools on temporary copies incl sidecars;
restored original HDR and regenerated error cases. Native importer is READ ONLY;
regression asserts input/header byte preservation and no partial output.
Limits in KNOWN_GAPS/README: finite standard headers/cells, fitting INTEGER/F32,
no compression/additional dialects/nondefault environments. CRS assignment separate.

## Validation / processes

- Initial clippy PASS /tmp/unigis-raster-import-clippy.log.
- Oracle generation /tmp/unigis-raster-import-generation.log, corrected short/missing
  generation /tmp/unigis-raster-import-short-generation.log; all expected outcomes.
- FINAL scripts/check PASS exit0 /tmp/unigis-raster-import-check.log: 180 passed,
  0 failed,5 ignored,81 groups;8 Python tests,fmt,workspace/all-target clippy-Dwarnings.
  Includes every import pixel exactly,125 Composite cases,48 Graphic Buffer cases,
  later-band corruption, closed-output and input-preservation guards.
- No running Cargo/ArcGIS processes. Importer source/docs/fixtures UNSTAGED currently.
- Root docs read; actual ug tools inventory confirmed both imports absent before work.

## Next actions

1. Refresh dashboard --ci, validate generated JSON/JS, stage only importer paths,
   commit and push normally through required gate. Verify remote SHA/CI.
2. Continue raster projection assignment: root define_projection currently opens
   features only (crates/unigis-tools/src/dm_tools.rs:225). New legacy imports produce
   unknown CRS by design, so this is a useful next capability. Probe ArcGIS first.
3. When working on raster CRS encoding, inspect geotiff.rs::is_geographic_code:
   current4000..4999 heuristic misses GDA2020 EPSG7844 (not tested/fixed yet).
   A pure EPSG classification table from public PROJ database could fix metadata
   without adding runtime GDAL/PROJ dependencies to the raster format engine.
4. Continue bounded tested slices/commits/pushes/dashboard, no repeated user prompts.

## Durable context / preserve

Research: scripts/research/{graphic_buffer,composite,grid_import,sa_batch}/README.md.
Read ROADMAP/KNOWN_GAPS before next scope. Registry+generator RUN+saved cases is the
new tool workflow. scripts/check pre-push mandatory, no bypass. Cargo-j3, no overlapping
builds, no local full app CI for root-only work. Post-commit rebuilds dashboard offline
from cached CI, leaving HTML dirty by design. Do not launch agents/coordinators.

Preserve/exclude unrelated aux.xml in testdata/geoproc/raster, deleted *.sr.lock,
.worktrees/, NOTES.md, scratch_cf.tif, scratch_trend_rms.txt.xml. DO NOT read/use NOTES.md
(proprietary reverse-engineering material). No reset/stash/worktree transfer or external
messaging performed.
