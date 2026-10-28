# Current handoff

Updated 2026-10-28T10:14:45.697693+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`; HEAD `e73e223` PUSHED and
verified remote `e73e223e66e2bfcbae7d099445ab2be4122f2a08`.
User authorizes continued tested development, frequent commits/pushes/dashboard.
Work alone; no new agents/CLI sessions/coordinators. Cargo -j3, sequential builds.

## Completed this session

Finished inherited Line Statistics from Claude usage-limit run
`.git/agent-coordinator/20261028T094627-4d2f2b6c`. Fixed zero-length circle contact
semantics and F32 vertex-offset clipping precision; added parameter/grid guards,
13 controlled cases (22 total), research and documentation. Commit e73e223.
No tolerance relaxed. Full scripts/check and normal pre-push hook PASS exit0:
187 Rust passed,5 ignored;8 Python;fmt/all-target workspace clippy-Dwarnings.
Logs /tmp/unigis-line-final-check.log and /tmp/unigis-line-push.log. Dashboard --ci,
node --check extracted script, diff --check passed. Limits in research README.

## Active Copy Raster statistics slice (UNCOMMITTED)

22 ArcGIS controls isolated cache presence from size/missing pixels/band count.
Default cached tagged inputs omit output stats; type/marker changes force stats;
explicit unchanged marker/type keeps omission. Untagged or uncached inputs write
stats. Source pixels/grid/CRS/NoData matched across six cached/uncached pairs.

Current edits: registry.rs replaces first-band missing/band-count heuristic with
source cache/tag and effective output type/marker checks. Generic raster comparer
adds cprstats_ to exact-float families. Generator has22 cprstats_* cases. Twelve
input snapshots +six genuine ArcGIS PAM sidecars under raster/copy_statistics;
recipe uses only GDAL file IO and preserves cached sidecars. Research scripts,
22 observations, policy explanation under scripts/research/copy_raster/statistics.
ROADMAP/KNOWN_GAPS/copy_raster README updated. All edits performed this session;
no inherited uncommitted source edits remain after Line Statistics commit.

## Current validation and next actions

- cargo fmt --all done. Twelve recreated inputs matched original oracle inputs
  pixels/grid/CRS/tags, only cached sources carry stats (read-only GDAL check).
- Oracle generation PASS all22 +exit0 /tmp/unigis-copy-statistics-references.log.
- scripts/check PASS exit0 /tmp/unigis-copy-statistics-check.log:187 Rust passed,
  5 ignored,0 failed;8 Python;fmt/all-target workspace clippy-Dwarnings. All22 new
  exact-copy cases and prior corpora/metadata passed.
- Dashboard --ci PASS exit0; extracted JavaScript node --check PASS. No push running.
- Independent overwrite probe confirms stale .aux.xml: native keeps count18000/
  mean238.346 from old DEM on replacement with small_cached, ArcGIS clears stats.
  Saved oracle evidence under statistics/probe_overwrite.py/overwrite_arcgis.jsonl.
  Candidate CLI regression /tmp/unigis-raster-output-statistics.rs; no cleanup
  source edit yet. Implement as next separate tested commit after policy push.

1. Commit staged policy slice, normal push with mandatory hook; verify remote.
2. Implement the documented overwrite-sidecar cleanup as a separate tested slice;
   start with /tmp/unigis-raster-output-statistics.rs regression before editing writer.
4. Remaining Copy Raster integer override/U32 signed-export differences are separate,
   documented gaps. Other roadmap work can follow another bounded milestone.

## Preserve unrelated files

Do not read/add NOTES.md or touch .worktrees/. Untracked geoproc/LineSta_roads*.tif
and sidecars are inherited exploratory oracle artifacts, excluded from commits.
Current-session LineSta_roads2 scratch output was removed after saving observations.
No reset/stash/worktree transfers. Post-commit dashboard dirt is expected.
