# Feature Class To Shapefile (conversion.FeatureClassToShapefile), ArcGIS Pro 3.7

Implemented as `feature_class_to_shapefile` (crates/unigis-tools/src/feature_class_to_shapefile.rs,
host capability `export_shapefiles` in the CLI). Reference cases `fc2shp_*` (folder outputs).

- Each input becomes `<input name>.shp` in the output folder; a name already in the folder or
  used earlier in the batch gets `_1`, `_2` (parcels twice -> parcels, parcels_1).
- Conversion is Feature Class To Feature Class's default (FileGDB classes and empty classes to
  shapefiles, fields as ArcGIS converts them).
- A missing input fails before anything is written: ERROR 000732.
- Not done: layers/selections, geodatabase names that are not valid shapefile names (not probed).
