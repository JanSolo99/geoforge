# Graphic Buffer reference coverage

ArcGIS Pro is the behavior oracle. The inherited `gb_*` cases in
`scripts/make-arcgis-reference.py` save ten ArcGIS shapefile results in
`testdata/arcgis-reference/`; each is exercised through `ug run graphic_buffer`
by the manifest-driven CLI comparison. No proprietary SDK or decompilation is used.
Public parameter reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/graphic-buffer.html

Observed cases cover projected points, lines and polygons, including a winding
line with tight bends and a polygon with a hole. They exercise square/butt/round
line caps and miter/bevel/round joins at the default miter limit. Round point
buffers at radius 15 have 36 segments, starting north (10-degree increments).
Round line/polygon corners need finer sampling to satisfy the existing buffer
comparison (0.2 map units, relative area tolerance 0.0001).

The native geometry implementation uses the existing `geo` offset engine, with
ring ownership retained for polygon holes. This replaces the unfinished band-and-
wedge implementation, which left gaps in tight bends and tried to remove holes
using an area/envelope heuristic. Those heuristics are no longer used.

Current limits: positive constant distances and planar 2D input only; curves/Z,
negative/zero distances, per-feature distance fields, maximum-deviation override,
and overwriting existing BUFF_DIST/ORIG_FID fields are not supported. Explicit
miter limits are accepted with finite values >=1, but nondefault limits still need
black-box probes. Nondefault geographic/coordinate-system environments and very
large-radius sampling remain unverified. Excessive arc vertex counts are refused.

Validation logs for the resumed slice:
- `/tmp/unigis-graphic-buffer-reference.log`: inherited implementation fails 5 cases.
- `/tmp/unigis-graphic-buffer-native-offset.log`: native offsets fix bends/holes;
  two round-join cases still fail area checks.
- `/tmp/unigis-graphic-buffer-round-refinement.log`: all ten cases pass after
  separating point-circle sampling from the finer line/polygon join sampling.
