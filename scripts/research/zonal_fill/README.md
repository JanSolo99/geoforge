# Zonal Fill (2026-10-31, ArcGIS Pro 3.7)

`random_cases.json`: 30 random 3-6 x 3-6 grids (zones 0..2, float weights 0..49) with ArcGIS's
output; `fit.py random_cases.json` scores candidate rules. Only this one fits all 30:

- A zone's value is the minimum, over pairs of 8-neighbours with one cell in the zone and one
  outside it, of the larger weight of the two;
- cells on the bottom and right raster edges also count, at their own weight (a neighbour past
  those edges); past the top and left edges they do not (4-neighbours with bottom/right fit 20,
  8-neighbours with all edges 15, a priority flood to the edge 14).
- A NoData weight drops out of the pair maximum; NoData zone cells count as another zone.
- Float weights give Float32 (NoData -3.4028235e38); integer weights the narrowest integer type
  (int32 weights with values up to 6 gave 4-bit, NoData 15).
