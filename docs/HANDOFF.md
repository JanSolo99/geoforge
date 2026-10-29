# Current handoff

Updated 2026-10-29T03:58:30.041025+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD
`5dcbe3d3e8b2d6e791a6b425c1c6aa49c1249861` (pushed before this task).

## Objective / current tool

Latest user: wrap the current tool, then build the next WORK_QUEUE item, one
landing per tool, scripts/check before commit/push. Prior tools already complete:
Assign Weights2fd0e8f and planar OD Links5dcbe3d. Do not repeat external actions
or polish landed tools. No additional agents/coordinators.

Current tool: Pairwise Polygon Neighbors. Implementation/reference comparisons
complete; full gate EXIT0.58 scoped paths staged, committing/pushing next.
No commit/push claimed yet.
- New crates/unigis-tools/src/pairwise_neighbors.rs uses existing native
  intersection/meeting; default OID or text/integer report tuples, area>edge>node
  hierarchy per aggregated pair, sorted output, BOTH_SIDES/NO_BOTH_SIDES, units.
- lib/registry wiring; registry area_in_units made pub(crate) without behavior
  changes. Classic neighbors.rs unchanged.
-14 ppn_* generator/RUN/TABLE_OUT/manifest cases:12 successful tables (176 rows),
  2 errors. Source fixture testdata/geoproc/pairwise_neighbors.* (12 polygons,
  edge/node/overlap/containment/duplicate/hole/multipart and repeated groups).
- Research recipes/results/compare.py/README in scripts/research/pairwise_neighbors/.
  Independent aligned-overlap and reversed-key probes completed; do not repeat.
-2 oracle unit controls and CLI output-preservation safety regression.
- ROADMAP/KNOWN_GAPS updated. Unvalidated options refused or recorded as gaps.

Key findings: Pairwise omits crossing/positive-area pairs without AREA_OVERLAP,
including aligned shared edges; independent50/25/4 area controls verify it.
Grouping sums contributions, then suppresses lower-order information when area
or edge totals exist. One-sided orientation follows report-value order rather
than OID order; independent reversed names/groups confirm it. Internal group
contacts appear once with doubled statistics for BOTH_SIDES. Saved schemas,
row ordering and units are verified; no comparison relaxations or skips added.

## Validation / running processes

- ArcGIS inputs/generated references completed: /tmp/unigis-ppn-inputs.log,
  /tmp/unigis-ppn-references.log (14 cases). Annotation complete. No probes remain.
- cargo build -j3 -p unigis-cli PASS, /tmp/unigis-ppn-build.log.
- python3 scripts/research/pairwise_neighbors/compare.py PASS14/14,176 rows,
  strict schema/text/count/order and standard numeric tolerance;
  /tmp/unigis-ppn-compare.log.
- cargo test -j3 -p unigis-tools pairwise_neighbors -- --nocapture PASS2,
  /tmp/unigis-ppn-unit.log. CLI safety PASS in full gate (5 refused option combinations; existing table
  and sidecars preserved).
- Python syntax (generator+4 research scripts), source/staged diff checks PASS.
  All2,173 previous manifest cases unchanged; exactly14 ppn_* additions.
- Full scripts/check EXIT0:217 Rust passed,5 ignored,Python8/fmt/clippy PASS.
  /tmp/unigis-ppn-check.{log,exit}. Covers final Rust and14 reference cases,
  all workspace tools, safety, GDAL formats, indexes, shapefile/geodatabase
  round trips. No build/probe running.
- Scoped staging audit PASS58 paths; /tmp/unigis-ppn-stage.patch excludes inherited
  raster generator hunk. Unrelated inherited hashes unchanged.

## Next actions

1. Commit/push only the validated Pairwise neighbors slice; gate/audits passed.
2. Record actual commit/push evidence and final handoff checkpoint.
3. Regenerate queue; next queue item: Remove Overlap (multiple). Leave polish in
   known gaps. No need to revisit previous tool landings.

## Inherited edits

Preserve raster unary integer-float local/registry/stats,unary safety,generator
umintfloat hunk,fixtures/research; coordinator/queue infrastructure AGENTS,hooks,
agents-remote,coordinator code/docs/tests,work_queue.py/WORK_QUEUE files,dashboard.
Baseline hashes /tmp/unigis-ppn-start-hashes.json and manifest
/tmp/unigis-ppn-start-manifest.json. Never read/add NOTES.md or .worktrees;
scratch_te/ scratch. No unrelated staging/commits and no overlapping builds.
