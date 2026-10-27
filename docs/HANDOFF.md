# Current handoff

Updated: 2026-10-27T15:12:24+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux, branch main, HEAD d29a79a pushed and verified
 d29a79ab0a4ba2944d446ade71f80ed9c40b5639. User authorizes ongoing commits/pushes.
Do not launch another agent, CLI session or coordinator.

## Objective

Sustained development, tested commits/pushes and dashboard updates while user sleeps,
without repeated continue prompts, until unable. Prior Claude usage-limit run recovered
from .git/agent-coordinator/20261027T101238-693a3583/. Basemaps/branding/docking already
landed; old chat requests are stale. Continue current root-engine development.

## Completed and pushed this Codex run

- d66383c: finish inherited Graphic Buffer caps/joins/holes (10 ArcGIS cases).
- ebc2d2d: dashboard implementation-aware reference coverage.
- cdc23aa: erosion/zero polygons, field replacement, miter bevel fallback (28 cases).
- c3abd4a: numeric/text distances, unit conversion and geographic/malformed-text
  guards (48 cases, 3 refusal tests). Limits in graphic_buffer/README.md.
- 735d4f3: persistent cached CI/timestamp/latest gaps; 8 dashboard tests.
- d29a79a: Composite Bands, 86 saved cases, all-band statistics/comparison and
  corruption regression. Full gate/push passed 178 tests +8 Python, fmt/clippy.
All pushed checkpoints have SUCCESS GitHub CI, including d29a79a:
https://github.com/JanSolo99/geoforge/actions/runs/40522528614

## Current slices (validated, committing next)

NoData / precision slice STAGED separately; CLI stdout fix UNSTAGED intentionally.

1. Composite Bands follow-up: make_matrix.py now writes typed GDAL/NumPy arrays and
   read-back verifies all 18 inputs (values/type/NoData/grid/EPSG). Prior AAIGrid
   inference narrowed F64/U32 fixture values; those earlier comparisons passed for
   actual stored inputs but missed intended precision boundaries. Corrected all
   matrix/padding oracles; U32 high values and true F64 fractions now checked exactly.
   29 new NoData cases => 115 Composite cases total. NoData defaults use highest-
   ranked tagged INPUT type (U8<I8<U16<I16<U32<I32<F32<F64), not output promotion.
   Defaults: 256 /256 /65536 /32767 /2147483647 /2147483647 /f32(3.4e38) /1.79e308.
   Preserve integer tags outside pixel range. Last U32 sentinel promoted to F64
   wraps signed32 (2^31 -> -2^31). Padding uses each source's sentinel or zero,
   independent of final global tag. Raw evidence/probes in research/composite.
   Source composite.rs, generator/manifest, matrix inputs, cbands_* outputs, research
   and docs are staged; unrelated dirt excluded.
2. Independent ug tools stdout fix in CLI main.rs and tests/tools_output.rs. Closed
   stdout previously panic/exit101 (/tmp/unigis-tools-broken-pipe-red.log). Direct
   Write catches BrokenPipe; deterministic closed-reader UnixStream test passes.
   Ordinary inventory still has all135 tools; bash pipefail ug tools | head exits0.
   Keep this as a SEPARATE commit after NoData commit.

## Validation

- make_matrix.py --check PASS all18 files (read-only).
- First NoData gate /tmp/unigis-composite-nodata-check.log FAILED only 2 default-tag
  cases; probes established tagged-input ranking; corrected and added 9 cases.
- FINAL scripts/check /tmp/unigis-composite-nodata-and-pipe-final-check.log PASS exit0:
  179 Rust passed, 0 failed, 5 ignored, 80 result groups; 8 Python dashboard tests,
  fmt and workspace/all-target clippy -D warnings. Covers both current slices,
  all115 exact Composite cases, 48 Graphic Buffer cases/guards and closed-output test.
- No running builds/ArcGIS workers. No other agent. Source frozen.
- Dashboard --ci fetched latest d29a79a SUCCESS; refresh after handoff/commits so its
  next-actions card reflects current state. Generated data/JS checks to repeat.

## Next actions

1. Commit staged NoData/precision slice after restaging this final handoff.
2. Commit CLI stdout fix separately (add roadmap checkpoint, handoff and dashboard).
3. Push both normally through mandatory pre-push gate, verify remote SHA and CI.
4. Continue bounded development. Remaining Composite probe: padding with an input
   integer NoData tag outside its own pixel range (not yet probed). Band selectors,
   custom CRS/reprojection/nondefault environments unsupported; packed/complex pixel
   types not verified. New tool candidates ASCII To Raster / Float To Raster.

## Durable context / preserve

Research: scripts/research/{graphic_buffer,composite,sa_batch}/README.md.
Read ROADMAP/KNOWN_GAPS before choosing next work. Capture ug tools to file for full
inventory; its closed-pipe panic is fixed in pending CLI slice. New tool workflow is
registry + generator RUN + saved cases. scripts/check pre-push is mandatory, no
bypass. Cargo -j3, no overlapping builds, no local full app CI for root-only edits.
Post-commit rebuilds dashboard offline from cached CI, leaving HTML dirty by design.

Preserve/exclude unrelated tracked aux.xml in testdata/geoproc/raster, deleted
*.sr.lock, .worktrees/, NOTES.md, scratch_cf.tif, scratch_trend_rms.txt.xml. DO NOT
read/use NOTES.md (proprietary reverse-engineering material). No reset/stash/worktree
transfer/external messaging/agent launch performed.
