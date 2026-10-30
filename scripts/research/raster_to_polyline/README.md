# Raster to Polyline — deferred (2026-10-30)

`arcpy.conversion.RasterToPolyline(in_raster, out_polyline_features, background_value=ZERO,
minimum_dangle_length=0, simplify=SIMPLIFY, raster_field)`; output fields `arcid`, `grid_code`,
`from_node`, `to_node`. Probes: synthetic integer rasters (1 m cells, origin 0,0) through the
ArcGIS bridge, ArcGIS Pro 3.7. Coordinates below are (col, row) of cell centres.

## Established (NO_SIMPLIFY and the graph)

- Cells of the same value are linked to 4-neighbours; a diagonal link exists only when neither
  shared 4-neighbour has the value (L corner (2,2),(3,2),(3,3) has no diagonal (2,2)-(3,3); a
  diagonal 1-px line is linked diagonally). Two-cell-wide lines are not thinned: they become
  ladders (every rung an arc).
- Nodes: cells with degree != 2, numbered 1.. in raster scan order (row by row, left to right).
  Isolated cells (degree 0) give nothing. A loop without nodes is one arc from node 1 to node 1.
- Arcs follow a scan: each cell attaches to visited neighbours (W before NW? / W before N, NW
  before NE observed); a chain is a cell list, a cell attaching to its head is prepended, to its
  tail appended, two chains meeting are concatenated. An arc is emitted (arcid 1..) when its chain
  closes at a node; arc direction is head -> tail. This reproduced every arc, direction and
  order of a 2-px bar crossed by a 1-px line (45 arcs), a U, a Y with diagonals and a ring —
  except the ring's start vertex is rotated one corner from this model's.
- Collinear vertices are always removed; grid_code is the cell value.

## Not identified: SIMPLIFY (the default)

1-px staircases, rows 1..4, runs of r cells per row (vertices at run ends):
- r = 2..5 and 10: one segment end to end (deviations up to 0.675 removed).
- r = 14 (first-corner deviation 0.708) and 15: no vertex removed at all.
- r = 20: (1,1) (60,3) (61,4) (80,4) — the first three steps merged, the last kept.
Douglas-Peucker with tolerance sqrt(2)/2 fits r <= 15 only partially and fails r = 20; forward
greedy (farthest or first failure), Reumann-Witkam strips and DP on 60-cell chunks each contradict
one of the cases. minimum_dangle_length and raster_field were not probed.
