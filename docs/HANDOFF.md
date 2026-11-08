# Current handoff

Updated 2026-11-08T04:27:11.689662+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, product HEAD `be04da46425d66c170703c9e0d06fd125d4f3ab6`.

## Single item: tool queue #3-#6 mosaic dataset tools — investigated, deferred (Claude, 2026-11-08)

Previous: Enable Editing Templates 3e92a912.
- Export Frame And Camera Parameters needs a frame-imagery mosaic; the other mosaic tools (Export
  Mosaic Dataset Geometry/Items/Paths, Import Mosaic Dataset Geometry, Remove Rasters, ...) read the
  mosaic catalog `AMD_<name>_CAT`. I built a real one with ArcGIS (`testdata/geoproc/md_items.gdb`,
  3 rasters) and established Export Mosaic Dataset Geometry's behaviour completely (where-clause
  rule, parts, fields, errors), but `unigis-filegdb` (and GDAL) cannot decode the catalog rows: the
  `Raster` column is raster type 2 (inline) and the row layout around the geometry/raster value is not
  understood. Implementing the tool was started and reverted; nothing of it is in the tree.
- Everything is recorded in `scripts/research/mosaic_datasets/README.md` (with a resume plan);
  ten mosaic tools are in `docs/WORK_QUEUE_DEFERRED.txt` with the reason.
- Next queue item: Export Raster World File (docs/WORK_QUEUE.md first non-deferred). Resuming the
  mosaic family needs a FileGDB reader task: decode `AMD_md_CAT` rows (row 1 at table offset 600).
- Inherited uncommitted work (raster umintfloat hunk in make-arcgis-reference.py, registry.rs/local.rs/
  stats.rs, dashboard.html, agent file, unary_band_safety) stays out.
