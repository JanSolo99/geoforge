# Add Spatial Join (management.AddSpatialJoin), ArcGIS Pro 3.7

The tool joins a layer; only `permanent_join=PERMANENT_FIELDS` leaves a trace on disk, so the
reference cases (`asj_*`) edit a shapefile copy in place and `add_spatial_join` writes a new
feature class with the same content.

- The permanent fields are the target's fields, then `Join_Count`, the optional distance
  field, then the join fields (renamed on collision as in Spatial Join); no `TARGET_FID`.
- Values equal Spatial Join JOIN_ONE_TO_ONE/KEEP_ALL with the default field mapping (first
  match in join-FID order; the distance field is -1 except for CLOSEST).
- KEEP_COMMON and JOIN_ONE_TO_MANY keep every target row and one match, but leave
  `Join_Count` and the join fields null on unmatched rows (blank in a shapefile); not modelled.
- In a shapefile, unmatched text join fields hold the string `0` (numbers 0); a geodatabase
  target holds nulls. We write nulls (blank text in a shapefile); the `asj_*` cases avoid
  unmatched text fields.
- Field-name collisions are shortened to the shapefile's 10 characters before the suffix
  (`PARCEL_I_1`); our Spatial Join engine names it `PARCEL_ID_1` and the writer truncates
  it to `PARCEL_ID_`. Same gap as Spatial Join into shapefiles.
