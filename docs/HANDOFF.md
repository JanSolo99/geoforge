# Current handoff

Updated 2026-11-08T04:27:11.689662+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, product HEAD `be04da46425d66c170703c9e0d06fd125d4f3ab6`.

## Single item: tool queue Get Raster Properties — implemented, validated, landing (Claude, 2026-11-08)

Previous: Get Cell Value 02c340e4.
- `unigis-raster::rasterprops` + `ug run get_raster_properties input=… property_type=… band_index=Band_1`
  prints the value. `worldfile::format_g(v, p)` generalises `%g`. Tests:
  `crates/unigis-cli/tests/get_raster_properties.rs` (33 ArcGIS values, 6 error cases) and a unit test.
  Evidence `scripts/research/get_raster_properties/`.
- Inherited uncommitted edits (umintfloat unary tests, ambition-review.md, dashboard, NOTES.md,
  scratch_te) are NOT part of this landing; stage by file/hunk (registry.rs has inherited hunks).

Next queue item: `python3 scripts/work_queue.py --next`.
