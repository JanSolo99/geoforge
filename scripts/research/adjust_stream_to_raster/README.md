# Adjust Stream to Raster / Adjust Raster to Stream — deferred (2026-10-31, ArcGIS Pro 3.7)

- Adjust Raster to Stream fails with ERROR 160327 ("A column was specified that does not exist") for
  shapefile lines, a file-geodatabase copy and a Stream To Feature network (arcid, grid_code,
  from_node, to_node) on testdata/raster/dem.tif, with or without out_path_features.
- Adjust Stream to Raster runs: output lines (field StreamLink, 1..) are chains of 8-connected cell
  centres on the raster's grid, extending beyond the raster's extent when the input does
  (roads.shp starts at x 499950, output at 499955); a self-crossing input line is split into two
  links; all cell-centre vertices are kept. The cells are not Polyline To Raster's cells on the same
  grid (wiggly line: 44 output cells vs 73 rasterized; every link adds 1-4 and drops 1-56), so the
  path tracing / sharp-angle removal (default REMOVE) is ArcGIS's own and was not identified.
  Group-by, divergent flow, the stream/flow-direction raster outputs and split points not probed.
