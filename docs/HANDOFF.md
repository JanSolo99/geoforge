# Current handoff

Updated 2026-11-08T04:27:11.689662+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, product HEAD `be04da46425d66c170703c9e0d06fd125d4f3ab6`.

## Single item: tool queue #1 Delete Raster Attribute Table — implemented, validated, landing (Claude, 2026-11-08)

All app priorities (docs/APP_PRIORITIES.md) are ticked; Codex's geodatabase design/relationship
work (be04da46) and naming/packaging work (e34da9a8) are landed, the docs checkpoint c8fc1f4d pushed.
- `ug run delete_raster_attribute_table input=… output=…` (registry entry for discovery; edits
  files, like Delete Colormap): requires `<raster>.vat.dbf` (else ERROR 000236), copies the raster
  and sidecars, drops `.vat.dbf`/`.vat.cpg`. ArcGIS evidence: `scripts/research/delete_raster_attribute_table/`.
- Reference cases `drat_aux`, `drat_vat_only`, `drat_none` (harness copies the `.vat.*` sidecars of
  an in-place raster tool); the reference test also compares whether `.vat.*` exist beside the output
  for `drat_` cases. Gaps (KNOWN_GAPS): only `ug run`, GeoTIFF `.vat.dbf` tables, the `.tif.xml`
  lineage entry is not written.
- Validation: `UNIGIS_REFERENCE_CASES=drat_` passes; full `scripts/check` result in the landing report.
- Staged by hunk: inherited raster umintfloat/covariance hunks in `registry.rs` and
  `make-arcgis-reference.py` stay uncommitted (with dashboard.html, agent file, unary_band_safety,
  local.rs, stats.rs).

Next queue item: Enable Editing Templates (docs/WORK_QUEUE.md #2).
