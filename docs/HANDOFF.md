# Current handoff

Updated 2026-10-29T04:36:31.824933+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD
`272b70f` (pushed; Split-only implementation staged252 paths, committing next).

## Objective / current state

User: finish/wrap current tool then build next WORK_QUEUE item, under time box
and one-tool-per-landing rules. Commit/push authorized after local validation.
Previous Pairwise neighbors338d0b9 complete; do not repeat or polish that landing.

Remove Overlap (multiple) DEFERRED:18 public ArcGIS controls saved in
scripts/research/remove_overlap/. Default CENTER_LINE gives a branched boundary
and areas325/275 for equal overlapping rectangles, unlike centroid/Thiessen
300/300. Native engine lacks a matching boundary partitioner. No runtime stub.
Deferral reason/README/known gap/roadmap recorded. A separate deferral-only docs
landing272b70f was committed/pushed (observed338d0b9..272b70f to origin/main). That landing excluded the user's prior AddIncrementingIDField entry; the
working copy retains it as an unstaged inherited change.

Next eligible tool, Split By Attributes: IMPLEMENTED, full gate EXIT0 (222 Rust passed,5 ignored; Python8/fmt/clippy PASS). No
product commit/push yet. Files/decisions:
- crates/unigis-tools/src/split_attributes.rs: pure grouping by non-null text/
  32-bit integer tuples, stable case-insensitive key order, source metadata/
  attribute/geometry/row-order retention and safe observed folder naming.
- registry Context has opt-in write_workspace and WorkspaceOutput. CLI Args
  implements native folder shapefile/DBF writes, allocates unique names before
  writing and preserves existing datasets. Other hosts remain source-compatible
  and explicitly fail without workspace capability; no app files changed.
- CLI reference comparator/generator workspace branches now include orphan DBFs
  and check command success even with empty output sets. Existing geometry and
  numeric tolerances unchanged.
-11 sba_* cases:8 successes,31 datasets,48 rows;3 errors. Source fixture families
  split_attributes.shp,split_attribute_names.shp,split_attribute_table.dbf under
  testdata/geoproc. Complete workspace reference folders saved.
- Research recipes/results/compare.py/README in scripts/research/split_by_attributes/.
  Three ArcGIS probe batches completed; reuse saved results. Existing outputs
  get suffix0,1,...; case collisions follow stable case-insensitive tuple order.
  ../path ->T___path; empty key ->T. Unsupported types/workspaces/hosts in gaps.
-3 unit tests and2 CLI safety tests added; ROADMAP/KNOWN_GAPS updated.

## Validation / running processes

- cargo build -j3 -p unigis-cli PASS /tmp/unigis-sba-build.log.
- cargo test -j3 -p unigis-tools split_attributes -- --nocapture PASS3,
  /tmp/unigis-sba-unit.log.
- python3 scripts/research/split_by_attributes/compare.py PASS11 cases/31
  datasets/48 rows (names/schema/attributes; geometry checked by full gate),
  /tmp/unigis-sba-compare.log. ArcGIS generation/annotation PASS11 cases,
  /tmp/unigis-sba-references.log. No ArcGIS probes remain running.
- Python syntax and manifest preservation PASS: all2,187 prior cases unchanged,
  exactly11 additions. Prior Rust/raster/coordinator edits preserved.
- Full scripts/check EXIT0:222 Rust passed,5 ignored; Python8/fmt/clippy PASS.
  /tmp/unigis-sba-check.{log,exit}. Covers all final Rust,11 references including
  geometry, table schemas/rows, CLI safety2, all prior tools and native format/
  shapefile/geodatabase round trips. No builds/probes remain running.

## Next actions

1. Full gate passed; Split-only252 paths staged and whitespace checked. Commit/
   push that implementation now; do not repeat deferral272b70f push.
2. Filtered staging patch /tmp/unigis-sba-stage.patch excludes inherited raster
   generator hunk. Old AddIncrementingIDField entry remains unstaged intentionally.
3. Regenerate queue; next queue item: Pairwise Integrate. Final handoff checkpoint
   after actual commit/push. No builds or probes running.

## Inherited edits

Preserve raster unary integer-float local/registry/stats,unary safety,generator
umintfloat hunk,fixtures/research and coordinator/queue infrastructure AGENTS,
hooks,agents-remote,coordinator code/docs/tests,work_queue.py/WORK_QUEUE files,
dashboard and old AddIncrementingIDField deferral. Baseline hashes
/tmp/unigis-rom-start-hashes.json. Never read/add NOTES.md or .worktrees;
scratch_te/ scratch. No agents/coordinators or overlapping Cargo builds.
