# Graphic Buffer reference coverage

ArcGIS Pro is the behavior oracle. The inherited `gb_*` cases in
`scripts/make-arcgis-reference.py` plus `gb_more_*` and `gb_field_*` cover 48 ArcGIS success/error cases in
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

Distance fields (`gb_field_*`, another 20 cases):
- Numeric fields and text fields containing numeric distances or recognized units
  are accepted, with case-insensitive field lookup. Numeric nulls and field zeros
  skip features with warning 000636. Negative point/line distances fail 000109.
- A missing field or unsupported constant unit gives 000728. A numeric prefix
  with an unknown unit *inside a text field* falls back to coordinate units.
- ArcGIS uses US survey definitions for Feet/Foot/FeetUS, Inches, Yards and Miles;
  FeetInt uses international feet. NauticalMiles is 1853.248 meters. Conversion
  changes are local to Graphic Buffer: other tools' aliases are not yet probed.
- Null or malformed text values can reuse the previous valid feature's buffer
  size while writing BUFF_DIST=0. We deliberately refuse these inputs, before
  writing any output. `field_observations.json` preserves the raw black-box rows.
- `make_fields.py` recreates the synthetic point fields with GDAL for file IO.
  Polygon field input: `ogr2ogr -f 'ESRI Shapefile'
  testdata/geoproc/graphic_buffer_poly_fields.shp testdata/geoproc/clip_areas.shp
  -sql 'SELECT *, -20.0 * AREA_ID AS DIST FROM clip_areas'`.

Current limits: planar 2D only; curves/Z and maximum-deviation override are
unsupported. Geographic input is refused, including raw numeric distances, so
meters cannot silently become degrees. Coordinate-system environments, M values,
incompatible reserved-field types and very large-radius sampling remain unverified.
Excessive arc vertex counts are refused. Tiny positive distances can collapse in
ArcGIS near its XY tolerance (the 1 Millimeters probe is empty); the native engine
may retain these shapes. No general collapse threshold has been established. A field literally named UNKNOWN
triggers ArcGIS parameter error 000735; that parser quirk is not yet reproduced.

Validation logs (local, ephemeral):
- `/tmp/unigis-graphic-buffer-reference.log`: inherited implementation fails 5 cases.
- `/tmp/unigis-graphic-buffer-round-refinement.log`: original ten cases pass.
- `/tmp/unigis-gb-more-probes.log`, `/tmp/unigis-gb-acute-probes.log`: 18 further
  ArcGIS probes, including three expected errors.
- `/tmp/unigis-gb-miter-overlap.log`: full manifest-driven CLI comparison passes,
  including all 28 Graphic Buffer cases, at existing tolerances.
- `/tmp/unigis-gb-field-probes.log`: 20 saved field/unit cases generated successfully
  (including five expected parameter/runtime errors).
- `/tmp/unigis-gb-fields-reference.log`: manifest comparison for the field/unit slice.
