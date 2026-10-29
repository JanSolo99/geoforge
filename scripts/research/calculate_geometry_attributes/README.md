# Calculate Geometry Attributes (management.CalculateGeometryAttributes), ArcGIS Pro 3.7

Implemented as `calculate_geometry_attributes` (crates/unigis-tools/src/calc_geometry.rs) on top
of Add Geometry Attributes' computations; in-place cases `cga_*` on fixtures with empty target
fields (`make_inputs.py`: cga_parcels/cga_roads/cga_wells.shp).

- `geometry_property` pairs are `FIELD KEYWORD`; target fields must exist. Keywords used: AREA,
  PERIMETER_LENGTH, LENGTH, CENTROID_X/Y, INSIDE_X/Y, EXTENT_MIN/MAX_X/Y, LINE_START_X/Y,
  LINE_END_X/Y, LINE_BEARING, PART_COUNT, POINT_COUNT, POINT_X/Y, HOLE_COUNT, CURVE_COUNT.
- Values go into the field's type (AREA into a Long field is rounded: 9216).
- Units: length_unit KILOMETERS, FEET_US, ...; area_unit HECTARES, ...; blank = the coordinate
  system's units.
- Not done: Z/M properties (POINT_Z, CENTROID_Z, LENGTH_3D, ...), geodesic lengths/areas,
  coordinate_system/coordinate_format (notation strings), text target fields.
