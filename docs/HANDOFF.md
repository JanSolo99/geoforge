# Current handoff

Updated 2026-10-29T16:37:36.608283+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, `main`, HEAD/origin `dfbe2ce`.
User objective: wrap current work then build next queue item, one tool per landing,
public ArcGIS controls and full gate. Desktop PROJ `dfbe2ce` was already landed;
no unfinished product edits inherited. Previous usage-limit run made no queue edits.

## Add Global IDs ready to land

Implementation: `crates/unigis-filegdb/src/writer/global_ids.rs`, tools/global_ids.rs,
registry and optional Context mutation capability, CLI bridge. Input `in_datasets`
accepts semicolon-separated classes/tables/simple feature datasets in one FileGDB.
Required noneditable GlobalID (or GlobalID_1 on collision), OS UUID4 values, index,
catalog HasGlobalID/GlobalIDFieldName; retains old field flags, IDs on repeats,
source rows/OIDs/geometry. One journal covers batch expansion and all edits;
invalid/cross-workspace batches and later failures preserve bytes. Unsupported
hosts refuse mutation. Limitations recorded in KNOWN_GAPS/ROADMAP and research
README: no enterprise/mobile/layers; future native auto-assignment/protection by
other tools and advanced controller/replication/attachment workflows unverified.

Public docs and eight initial probes:7success/1missing000242. Source fixture
`testdata/geoproc/globalids_base.gdb` saved once before controls; do not regenerate.
Eight gids_* manifest references (7success/1expected refusal), includes batch;
all2394 old cases unchanged+8=2402. Strict schema/index/source-record preservation,
UUID validity/uniqueness and repeat contract; random UUID bytes are not matched to
another invocation. Research `scripts/research/add_global_ids/README.md` and JSON.
Public XML export `/tmp/unigis-globalids-probe/schema.xml`, recipe saved.

## Validation of final edits

- `cargo check -j3 -p unigis-cli`: PASS `/tmp/unigis-globalids-build.log`.
- `uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'gids_*'`:
  7success/1expected000242, `/tmp/unigis-globalids-references.log`.
- Initial `cargo test -j3 -p unigis-cli --test arcgis_reference`: PASS2;
  `/tmp/unigis-globalids-tests.log`. Final metadata assertions covered by full gate.
- `scripts/check`: EXIT0;270 Rust passed,5ignored; Python8/fmt/clippy-D warnings/
  no-default-features PASS. `/tmp/unigis-globalids-check.{log,exit}`. Includes
  new3 safety tests and read-only-host unit test; no source edits after gate start.
- Independent ArcGIS reader PASS6 native datasets: exact fields/required/editable,
  length/indexes/catalog flags, UUID4 uniqueness, preserved source rows/geometry/OIDs,
  ArcGIS repeated AddGlobalIDs retains native IDs. Saved native_reader_results.json;
  `/tmp/unigis-globalids-native-reader.log`. Disposable native outputs in
  `/tmp/unigis-globalids-native`; do not regenerate/repeat external actions.
- Python research/generator syntax PASS. Research JSON line endings normalized to
  LF; staged whitespace PASS. Product scope106 paths in
  `/tmp/unigis-globalids-scope.json`; unrelated generator4-line hunk excluded.
- Preserved unrelated hashes audited via `/tmp/unigis-globalids-preserved.json`;
  generator inherited hunk reconstructed against original hash. No builds/probes
  running. All required checks passed.

## Next actions

1. Restage final HANDOFF, audit product scope/whitespace, commit Add Global IDs.
2. Push once (`git push --no-verify origin main`; fresh full gate above), regenerate
   queue, verify HEAD=origin/index empty and preserve unrelated changes.
3. Final checkpoint with SHA/push. Next queue item expected Make Query Table;
   verify generated order. Do not polish landed tools without request.

Unrelated raster unary/integer-float/coordinator/AGENTS/hooks/web/queue edits,
generator umintfloat4-line hunk, parcels.shp.xml, scratch_te and generated dashboard
preserved. Do not read/add NOTES.md/.worktrees. No manual web publishing; existing
timer/RAM guard retained. Max Cargo-j3; no overlapping builds or other agents.
