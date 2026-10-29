# Create Spatially Balanced Points — deferred

Public ArcGIS Pro3.7 controls in probe.py/probe_results.json. Seed23 and uniform
16x16 probability raster:4 samples occupy four different coarse quadrants; the
8-sample result contains all four4-sample sites. Repeating seed23 matches exactly.
Weighted and sparse masks also succeed; values outside[0,1] fail045037 and count0
fails000854. Points are cell centers, with InclProb Single in the output.

The public algorithm is Reverse Randomized Quadrant-Recursive Raster (RRQRR),
including unequal inclusion probabilities and nested/repeatable fixed-seed designs.
The native engine has no RRQRR design; ordinary weighted sampling would not meet
these properties. Raster registry results are rasters, while the feature Context
has no probability-grid input capability. This needs a real raster-to-feature
sampling design/result bridge, beyond a small per-tool landing. Defer rather than
registering a weighted random substitute. All source/output rasters are disposable
under /tmp/unigis-balanced-probe; no SDK or dataset edits.

References:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-spatially-balanced-points.html
https://doc.esri.com/en/arcgis-pro/latest/help/analysis/geostatistical-analyst/how-create-spatially-balanced-points-works.html
