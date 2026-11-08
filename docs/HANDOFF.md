# Current handoff

Updated 2026-11-08T04:27:11.689662+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, product HEAD `be04da46425d66c170703c9e0d06fd125d4f3ab6`.

## Single item: tool queue Export Raster World File — implemented, validated, landing (Claude, 2026-11-08)

Previous: mosaic tools deferred c9ce3cb9.
- `unigis-raster::worldfile` + `ug run export_raster_world_file input=… output=…`: copies the raster
  and sidecars to `output`, writes `<output>.tfw` (`.igw`, `.j2w`, ... by first+last extension letter)
  with ArcGIS's format: six `%.18g` numbers, CR LF, cell size derived from the extent
  (geographic rasters print 9.99999999999978635e-05). Evidence: `scripts/research/export_raster_world_file/`.
- Tests: unit tests in `worldfile.rs`; `ewf_dem/geographic/u8/nonsquare/existing` compare the file
  text with ArcGIS; `crates/unigis-cli/tests/export_raster_world_file.rs` (missing input,
  ERROR 000732). New fixture `testdata/raster/world_nonsquare.tif` (ArcGIS-made).
- Gaps (KNOWN_GAPS): `ug run` only, GeoTIFF inputs, rotated refused, lineage xml not written.
- Validation: `UNIGIS_REFERENCE_CASES=ewf_` passes; full `scripts/check` result in the landing report.
  Staged by hunk: inherited raster/umintfloat/dashboard/agent edits stay uncommitted.

Next queue item: see `python3 scripts/work_queue.py --next` (Export XML Workspace Document #8 or
Get Cell Value / Get Raster Properties next in the list).
