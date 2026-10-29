# Current handoff

Updated 2026-10-29T13:28:11.607574+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main; HEAD/origin main on startup and before
landing `61c5ea8ec866abcbaecd1f36b81f5428f1c68497`.

## Objective and current state

User: finish/wrap the tool in progress, then build the next WORK_QUEUE tool with
public ArcGIS controls, reference cases, full gate and one scoped landing.
Prior tools already pushed on startup; Create Database View deferred in61c5ea8.
This turn implemented Extract Data From Geodatabase.981 scoped paths staged;
full gate PASS, ready to commit/push. No incomplete product edits or running builds.
No additional agent/coordinator or external web publish launched.

## Implementation and evidence

- `crates/unigis-filegdb/src/writer/extract.rs`: extraction from one FileGDB into
  new/existing FileGDB, DATA/SCHEMA_ONLY, standalone classes/tables. Preserves
  source field storage schema/defaults/aliases/domains/subtypes, coordinate
  resolution, values/NULLs and OID gaps; copies single-field indexes and simple
  relationships. Existing writes journaled; new outputs cleaned on failure.
- `crates/unigis-tools/src/extract_data.rs`, optional Context host capability and
  CLI host. Unsupported hosts fail explicitly. Default unfiltered tables are
  schema-only, but GET_RELATED follows simple key relationships in both directions.
  Expansion/default/all rows/DO_NOT_ADD covered. Source names matched case-insensitively.
- ArcGIS dynamic default discovered: ADD_WITH_SCHEMA_ONLY with omitted related
  flag yields0 observations; explicit GET_RELATED yields3. Implemented/tested.
  First native reference run missed this one case; focused public probe resolved it.
- Empty high-precision Date fields preserve metadata when no fractional rows are
  written (`finish_table` retains existing precision flags). Safety regression added.
- Feature datasets, advanced/OBJECTID-key relationships, tracking/COGO/attachments,
  multiple sources, geometry filters, schema reuse, XML/mobile outputs refused.
  Index names retained where ArcGIS regenerates DSID names; class-level alias/
  extension metadata not copied. Further unverified scope in KNOWN_GAPS and
  `scripts/research/extract_data/README.md`; no polishing requested.
- Public immutable source `testdata/geoproc/extract_base.gdb` created/saved once
  this turn; do not recreate. Public controls/results/default probe in
  `scripts/research/extract_data/`.16 edg_* references (14 success,2 expected errors).
 2323 old manifest cases unchanged;2339 total. Whole-workspace comparator checks
  catalog/fields/defaults/precision/domains/subtypes/relations/index expressions/
  OIDs/rows/geometry/CRS. ROADMAP and KNOWN_GAPS updated.

## Validation

- `uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'edg_*'`
  PASS14 success/2 expected000732/000735 refusals; /tmp/unigis-extract-references.log.
- Targeted `cargo test -j3 -p unigis-cli --test extract_data_safety --test arcgis_reference`
  PASS5 (2 refs,3 safety) /tmp/unigis-extract-tests2.log. Fourth dynamic-default
  safety test added afterwards and covered in full gate.
- Independent public arcpy reader PASS ALL14 native outputs: exact field properties/
  aliases/defaults/domains/rows/OIDs/geometry/relationships/indexed fields. Saved
  native_reader_results.json; /tmp/unigis-extract-native.{log,json}. Disposable native
  outputs /tmp/unigis-extract-native. No output rerun needed.
- Final `scripts/check` EXIT0, /tmp/unigis-extract-check.{log,exit,pid}: 250 Rust
  passed, 5 ignored; Python8/fmt/clippy -D warnings/no-default-features check PASS.
  Includes all4 new safety tests and host refusal test, full ArcGIS references and
  format/engine suites. Two initial gates stopped on clippy collapsible_if lints;
  corrected, prior logs saved *-first/second.log/exit. No behavior changes after
  reader validation; no product changes after final gate started.
- Python syntax, artifact/no-lock audits, staged whitespace/scope and inherited
  hash preservation PASS. Research JSON normalized Wine CRLF to LF with data intact.

## Next actions

1. Commit the981 scoped staged paths, then push origin main (fresh full gate above;
   avoid rerunning duplicate pre-push gate).
2. Run python3 scripts/work_queue.py and record the actual next item/HEAD/push.
3. Final checkpoint before stopping. No further product work needed for this tool.

## Inherited unrelated work (preserved)

Raster local/registry/stats and unary safety/fixtures/research, generator4-line
umintfloat hunk, AGENTS/hooks/agents-remote/coordinator, web publisher RAM guard/
docs/service/timer, queue infrastructure and generated dashboard/WORK_QUEUE.
These remain outside tool commit. Generator index excludes only inherited block.
/tmp/unigis-extract-preserved-hashes.json protects unrelated tracked files; old
/tmp/unigis-dm-inherited-hashes.json differs only for prior intentional WEB edits.
Never read/add NOTES.md or .worktrees; scratch_te is scratch. Cargo -j3 max;
no overlapping Cargo/Trunk; no manual web publish. /tmp currently5.7GiB available.
Previous desktop-PROJ request log showed only read-only startup then usage limit,
no edits; current user's queue request took precedence.
