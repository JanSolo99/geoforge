# Adjust 3D Z (management.Adjust3DZ), ArcGIS Pro 3.7

Implemented as `adjust_3d_z` (crates/unigis-tools/src/adjust_z.rs); in-place reference cases
`a3z_*` on testdata/shp/{pointz,polylinez,polygonz,multipointz}.shp.

- z' = (z + adjust) * factor, then negated with REVERSE (REVERSE, 10, METERS->FEET on z=3 gives
  -42.650833). The adjust value is a number or a numeric field; a null field value counts as 0.
- Units are US survey based: 1 m = 39.37 in, foot 1200/3937 m, yard 3 ft, fathom 6 ft
  (METERS->FEET factor 3.2808333...; FEET->MILLIMETERS 304.8006096).
- A feature whose new z would fall outside the feature class's z domain is left unchanged with
  WARNING 002631 (METERS->MILLIMETERS on small shapefile domains); not modelled.
