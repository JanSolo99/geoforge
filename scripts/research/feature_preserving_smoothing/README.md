# Feature Preserving Smoothing — deferred (2026-10-31, ArcGIS Pro 3.7)

Parameters: distance_units CELLS, neighborhood_distance 5, normal_difference_threshold 15,
number_iterations 3, maximum_elevation_change (unset), z_unit METER, analysis_target_device
GPU_THEN_CPU. On testdata/raster/dem.tif: Float32, NoData 3.4e38, mean |change| 0.0968 m, max
0.4999 m.

`prototype.py` (Lindsay et al. 2019 as described: Horn normals, normal smoothing in a 5-cell disk
with weights (cos - cos 15°)², 8-neighbour plane projection, 3 iterations, change cap) lands farther
from ArcGIS (mean 0.197 m) than the unsmoothed DEM, so ArcGIS's normal estimation, weighting or
update differs; matching to the reference tolerance (~2 mm) was not reachable in the time box.
