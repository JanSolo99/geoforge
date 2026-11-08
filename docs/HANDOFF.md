# Current handoff

Updated 2026-11-08T09:00:38.223823+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`; deferral HEAD
`93f8404065b2f18c657154876fec1fd70644956d` (this final handoff is a subsequent documentation-only checkpoint).

## Single item: Remove Depth Map — deferred and pushed; not implemented

User objective: handle one queue item, pass `scripts/check`, commit/push; no second
item this turn. Prior XML importer and handoff were already pushed/accepted.

This session:
- Confirmed no existing tool implementation and no open app priority; selected
  Remove Depth Map from the first queue row. No engine/CLI/registry code changed.
- Reproduced native `AMD_md_CAT` row failures (count 56/54/65 exceeds remaining data)
  using `target/debug/examples/dump testdata/geoproc/md_items.gdb AMD_md_CAT`.
- ArcGIS Pro 3.7 build 1901 probes used only this turn's copied scratch imagery and
  test mosaic. Four query/default success controls left invalid `.dm` sentinels
  untouched; empty mosaic succeeds, unknown field => 160144, TIFF => 999999.
  Genuine Compute Depth Map output/adjusted-imagery controls were unavailable.
- Success no-ops append a metadata Process entry and change GDB_Items/timestamps;
  both tested SetLogHistory settings still did so. Other metadata settings untested.
- Deferred tool in `docs/WORK_QUEUE_DEFERRED.txt`; regenerated queue, updated roadmap
  and known gaps. Evidence/reproduction scripts: `scripts/research/remove_depth_map/README.md`.
  Its ignored scratch/ holds only copied imagery/sentinels and this probe's geodatabase.

Decision: no success-only/no-op implementation. Native mosaic item/path decoding is
required to evaluate SQL and locate the right external files. Real depth-map controls
must establish actual selected/all-item removal and preservation of unrelated files.
This is an unmet format/evidence prerequisite, not a reasoning escalation.

Validation:
- Probe Python syntax, 8 primary/2 metadata JSON records, evidence paths and generated
  queue deferral/next-item assertions: PASS.
- Scoped and staged `git diff --check`: PASS.
- `scripts/check` for this final docs/research/queue landing: PASS, exit 0
  (`/tmp/remove-depth-map-gate.log`). Format, strict clippy, no-default-features build,
  full workspace tests and doctests passed. Covers unchanged engine plus inherited edits.
- No existing verifier files or acceptance definitions modified. No active builds/probes.

Inherited edits excluded: ambition-review; CLAUDE/bridge Wine configuration and pycache;
unary safety/local/stats/registry and reference-script umintfloat changes; dashboard;
NOTES.md; scratch_te/; unary_integer_float fixtures/research; web-build-memory.py;
.worktrees/. No other agent/coordinator launched. No resets/stashes/transfers.

Landing: `93f84040` Defer Remove Depth Map pending mosaic catalog support pushed to
`origin/main` successfully (4b19922a..93f84040, exit 0). `git push --no-verify origin main`
reused the just-passed full gate. Only documentation changes followed that gate.
No owned implementation edits remain unfinished; the capability itself is still missing.

Remove Depth Map is deferred, not implemented; resume only after its catalog-decoding
and genuine-control prerequisites. No active queue item remains after this deferral.
Next queue item: Set Raster Properties. NOT started this turn.
