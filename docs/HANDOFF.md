# Current handoff

Updated 2026-11-08; last agent Claude (coordinator repair, no product work).
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`.

## Status

Nothing is in progress. Next item: app priority **catalog-no-freeze** (top of
`docs/WORK_QUEUE.md`; details in `docs/APP_PRIORITIES.md`).

Recently landed:
- `81d1ced` Run geoprocessing tools with an empty map (app.gp-empty-map; ticked in
  APP_PRIORITIES). Validation (Codex, 2026-11-07): three engine regressions passed,
  root `scripts/check` passed, native desktop Catalog → New → File Geodatabase
  succeeded with an empty map.
- `0f0be36` Add Surface Parameters.

## Known app-workspace failures (not caused by recent work)

Recorded in `docs/KNOWN_GAPS.md` (Process gaps): in the separate `app/` workspace,
`cargo xtask ci` fails on six plugin WASM tests (`serde_json/preserve_order` from the
merged CLI breaks the fixture's byte offsets) and two UniGIS adapter tests
(`one_tool_list` duplicate IDs; `field_tools_work_in_place_and_others_make_layers`).
Root `scripts/check` passes. Use `CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target`
for app builds (rustc temp-file errors on NTFS).

## Coordinator note

The coordinator (`~/unigis-coordinator`) looped overnight on 2026-11-07: workers were
told to wait for acceptance instead of landing, so 98 turns re-verified gp-empty-map.
Fixed: workers land their own work (commit/push per `docs/WORK_QUEUE.md`), are told when
their previous task was accepted, and the daemon pauses after two accepted turns that
change nothing. Do not wait for "coordinator acceptance": land, then finish.

## Preserved, not validated

Uncommitted from earlier sessions: raster `src/{local,registry,stats}.rs`,
`crates/unigis-cli/tests/unary_band_safety.rs`, `scripts/make-arcgis-reference.py`,
`.claude/agents/ambition-review.md`, other `app/` edits (i18n, packaging); untracked
`.worktrees/`, `NOTES.md`, `scratch_te/`, `scripts/research/unary_multiband/integer_float/`,
`scripts/web-build-memory.py`, `testdata/raster/unary_integer_float/`.
