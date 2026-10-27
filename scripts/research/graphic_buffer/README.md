# Graphic Buffer reference coverage

ArcGIS Pro is the behavior oracle. The inherited `gb_*` cases in
`scripts/make-arcgis-reference.py` plus `gb_more_*` cover 28 ArcGIS success/error cases in
`testdata/arcgis-reference/`; each is exercised through `ug run graphic_buffer`
by the manifest-driven CLI comparison. No proprietary SDK or decompilation is used.
Public parameter reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/graphic-buffer.html

Observed cases cover projected points, lines and polygons, including a winding
line with tight bends and a polygon with a hole. They exercise square/butt/round
line caps and miter/bevel/round joins with miter limits 1, 1.25, 2 and 10. Round point
buffers at radius 15 have 36 segments, starting north (10-degree increments).
Round line/polygon corners need finer sampling to satisfy the existing buffer
comparison (0.2 map units, relative area tolerance 0.0001).

The native geometry implementation uses the existing `geo` offset engine, with
ring ownership retained for polygon holes. This replaces the unfinished band-and-
wedge implementation, which left gaps in tight bends and tried to remove holes
using an area/envelope heuristic. Those heuristics are no longer used.

Further black-box probes establish:
- Negative polygon distances erode, and zero polygon distances produce empty output.
  Collapsed features warn with 000636; empty output warns with 000117.
- Negative/zero point distances fail with 000109/000026. Point BUTT caps fail with
  000800 (SQUARE | ROUND); this cap is valid for lines.
- Existing numeric BUFF_DIST/ORIG_FID fields retain their positions and schemas,
  with values replaced. Incompatible field types are not yet probed.
- Round point radii 15/30/1000 produce 36/39/223 segments.
- ArcGIS bevels a corner beyond the miter ratio; the native library clips it.
  We start from bevel offsets and add allowed tips (subtract for erosion).
  Patches include the original corner to overlap the base, and are unioned
  before the binary operation to avoid cancellation where tips overlap.
  Acute synthetic lines and fractional miter limits catch these distinctions.

Input fixture `graphic_buffer_angles` contains three synthetic bends (acute,
moderate and right-angle). `graphic_buffer_reserved` copies wells and adds numeric
BUFF_DIST/ORIG_FID fields. GDAL creates these input files only, with no geoprocessing
behavior used as a reference.

Current limits: constant distances and planar 2D input only; curves/Z, per-feature
distance fields and maximum-deviation override are unsupported. Geographic and
coordinate-system environments, M values, incompatible reserved-field types and
very large-radius sampling remain unverified. Excessive arc vertex counts are refused.

Validation logs (local, ephemeral):
- `/tmp/unigis-graphic-buffer-reference.log`: inherited implementation fails 5 cases.
- `/tmp/unigis-graphic-buffer-round-refinement.log`: original ten cases pass.
- `/tmp/unigis-gb-more-probes.log`, `/tmp/unigis-gb-acute-probes.log`: 18 further
  ArcGIS probes, including three expected errors.
- `/tmp/unigis-gb-miter-overlap.log`: full manifest-driven CLI comparison passes,
  including all 28 Graphic Buffer cases, at existing tolerances.
