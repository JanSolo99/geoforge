# Current handoff

Updated 2026-10-28T10:29:55.514367+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; main; HEAD `a613de7` PUSHED/verified remote
`a613de77564b6e57803f03481a23234a4d52be69`.
User authorizes continuous tested development, frequent commits/pushes/dashboard.
Work alone; no new agents/CLI sessions/coordinators. Cargo -j3, sequential builds.

## Completed this session

- e73e223 Line Statistics: finished inherited Claude edits, corrected circle
  contacts and clipping precision, all8 statistics/int+double fields, parameter/
  grid guards.22 saved ArcGIS cases, research/recipes and failure regression.
- a613de7 Copy Raster stats:22 controlled cases/12 input snapshots isolate source
  cache/tag and effective type/marker changes. Replaces first-band missing/band-
  count heuristic. Exact cells/type/NoData and per-band metadata references.
- Both committed and normally pushed with hooks; each full gate187 Rust passed,
  5 ignored,0 failed;8 Python;fmt/all-target workspace clippy-Dwarnings. Dashboard
  --ci and extracted JS node --check PASS. No tolerance relaxed. Roadmap/gaps updated.
  Logs /tmp/unigis-line-final-check.log, /tmp/unigis-line-push.log,
  /tmp/unigis-copy-statistics-check.log, /tmp/unigis-copy-statistics-push.log.

## Active overwrite-statistics fix (UNCOMMITTED)

Native CLI reproduces stale .aux.xml: copy dem.tif (stats count18000/mean238.346),
then small_cached to same output (3x2 pixels) and old DEM statistics survive.
ArcGIS clears stats on this replacement. Evidence and probe committed under
scripts/research/copy_raster/statistics/{probe_overwrite.py,overwrite_arcgis.jsonl}.
Known gap recorded. Target write_raster_with in CLI main.rs: after successful
TIFF write, remove old .aux.xml when statistics=false; tolerate absence, report
other IO errors. Keep validation failures before mutation. Other sidecars not scoped.

Current edits: new crates/unigis-cli/tests/raster_output_statistics.rs exercises
replacement, in-place copy and invalid-parameter preservation of TIFF+aux bytes.
RED regression FAILED as expected at stale-statistics assertion, exit101,
/tmp/unigis-raster-overwrite-before.log. Implemented cleanup in main.rs; targeted
regression PASS exit0 /tmp/unigis-raster-overwrite-after.log. Full scripts/check PASS exit0 /tmp/unigis-raster-overwrite-check.log:188 Rust
passed,5 ignored,0 failed;8 Python;fmt/all-target workspace clippy-Dwarnings.
Dashboard --ci exit0, extracted JS node --check and diff --check PASS.
No push running. Independent ArcGIS in-place control failed999999/mutated scratch;
retained native existing in-place extension, documented difference/probe. No new
same-path restriction added. Handoff/dashboard dirt expected from checkpoint/post-commit.

## Next actions

1. Commit cleanup/regression/docs (all checks passed), normal push with hook.
2. Verify remote SHA before next source edits. Known gap resolved; native in-place
   extension explicitly documented because ArcGIS same-path probe corrupts scratch.
3. Verify remote SHA; checkpoint before choosing next roadmap/gap slice.
Copy Raster tagged-U32 signed exports/extreme override mapping remain separate.

## Preserve unrelated files

Do not read/add NOTES.md or touch .worktrees/. Inherited geoproc/LineSta_roads1.*
are excluded exploratory artifacts. Current-session LineSta_roads2 scratch was
removed after saving observations. No reset/stash/worktree transfers. No full
local app CI for root-only changes. Never bypass push hook.
