# Current handoff

Updated 2026-11-08T09:41:36.736984+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `7076f78633915806e592f0b9097a27bf38974762`.

## Single item: tool queue Set Raster Properties — implemented, landing (Claude, 2026-11-08)

Previous: Get Raster Properties; Remove Depth Map deferred.
- `unigis-raster::setprops` + `ug run set_raster_properties in_raster=… data_type=… statistics="band min max mean std"
  nodata="band value"` (in place). 26 `srp_*` references in the manifest; tests
  `crates/unigis-cli/tests/set_raster_properties.rs` and the arcgis_reference srp_ comparison.
- Fixed this turn: reference test no longer appends a bare output path for in-place registry
  tools; untouched bands lose an empty STATISTICS_EXCLUDEDVALUES like ArcGIS; manifest regenerated.
- Inherited edits excluded (stage by hunk): ambition-review, CLAUDE.md/bridge, unary safety/local/stats,
  umintfloat registry/reference hunks, dashboard, NOTES.md, scratch_te, unary_integer_float fixtures.

Next queue item: `python3 scripts/work_queue.py --next`.
