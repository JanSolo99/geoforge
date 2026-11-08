# Export Raster World File (Data Management) — ArcGIS Pro 3.7

Probed 2026-11-08 on `testdata/raster` (dem, dem_geographic, classes_u8, Combina_patches1 which
already has a `.tfw`, a 20 x 10 non-square raster) and copies in IMG, PNG, JPEG, BMP, GIF, JP2.

Observed:
- One parameter (`in_raster_dataset`); the world file is written beside the raster and replaces an
  existing one of the same name (a `.wld` next to a `.tif` is left alone). The raster's `.tif.xml`
  metadata gains a lineage entry (not reproduced).
- Extension: first and last letter of the raster's extension plus `w`: tif tfw, img igw, jp2 j2w,
  png pgw, jpg jgw, bmp bpw, gif gfw.
- Content: six lines with CR LF endings, each printed as C's `%.18g`: cell width, `0`, `-0`, negated
  cell height, then x and y of the centre of the upper-left cell. The cell size is derived from the
  extent: 150 columns of 0.0001 degrees give `9.99999999999978635e-05` (and the height
  `-0.000100000000000003786`), not the stored 0.0001.
- A missing path: `ERROR 000732: Input Raster Dataset: Dataset <path> does not exist or is not
  supported`. A shapefile input succeeds without writing a world file (odd; not reproduced).

Implemented for GeoTIFF inputs in `unigis-raster::worldfile` and `ug run export_raster_world_file`;
reference cases `ewf_*` compare the file text with ArcGIS's.
