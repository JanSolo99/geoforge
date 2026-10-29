# Current handoff

Updated 2026-10-29T04:33:58.069293+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD
`338d0b900cc8caac226baa729fd4d8df19be993f` (pushed before this task).

## Objective / current state

User: finish/wrap current tool then build next WORK_QUEUE item, under time box
and one-tool-per-landing rules. Commit/push authorized after local validation.
Previous Pairwise neighbors338d0b9 complete; do not repeat or polish that landing.

Remove Overlap (multiple) DEFERRED:18 public ArcGIS controls saved in
scripts/research/remove_overlap/. Default CENTER_LINE gives a branched boundary
and areas325/275 for equal overlapping rectangles, unlike centroid/Thiessen
300/300. Native engine lacks a matching boundary partitioner. No runtime stub.
Deferral reason/README/known gap/roadmap recorded. A separate deferral-only docs
index is staged (7 paths); no commit/push yet. Index intentionally omits the
user's earlier untracked AddIncrementingIDField deferral; working copy retains it.

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

1. Gate passed. Commit/push deferral-only staged7 files; index excludes Split code and the old user deferral entry.
2. Stage/commit/push Split implementation and evidence separately. Filter the
   shared generator diff to exclude inherited umintfloat hunk. Do not stage the
   user AddIncrementingIDField deferral or unrelated queue/coordinator files.
3. Regenerate queue; next queue item: Pairwise Integrate. Leave polish in gaps.

## Inherited edits

Preserve raster unary integer-float local/registry/stats,unary safety,generator
umintfloat hunk,fixtures/research and coordinator/queue infrastructure AGENTS,
hooks,agents-remote,coordinator code/docs/tests,work_queue.py/WORK_QUEUE files,
dashboard and old AddIncrementingIDField deferral. Baseline hashes
/tmp/unigis-rom-start-hashes.json. Never read/add NOTES.md or .worktrees;
scratch_te/ scratch. No agents/coordinators or overlapping Cargo builds.
