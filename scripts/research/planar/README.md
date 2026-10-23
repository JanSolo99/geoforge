# Planar graph tools: ArcGIS Pro 3.7 probes

`helpers.py` is exec'd in the arcpy session (`arcgis_python`, or `tools/arcgis-bridge/run_code.py`)
to build small shapefiles (UTM 32N, offsets from 500000/5000000) and dump tool outputs; the dumps
are in `probes/p*.json` (`{case: {in, out}}`, coordinates relative to the offset). `run.py` runs
`ug` on the same inputs and compares vertex by vertex (`cmp.py`); it expects the probe shapefiles
in a `pg/` folder next to it. `ph.py` / `ph3.py` replay the Feature To Polygon hole placement
(sweep diagonals and walk variants) against the dumps.

Findings (implemented in `crates/unigis-tools/src/planar.rs`):

- Coordinates within 2√2 × XY tolerance merge at their mean (0.0028 m merges, 0.0029 m does not;
  probes `c*`, `e*`).
- Row order everywhere: top-right vertex (max y, then max x), then the direction leaving it,
  counterclockwise from west; ties by attributes (P2L: LEFT_FID, RIGHT_FID; FTL: FIDs per input).
  Probe `d1` separates top-right from top-left; `t7` angle from next-vertex x.
- Polygon To Line IDENTIFY_NEIGHBORS: nodes at degree ≠ 2, owner changes and every ring start
  (`t2`, `t8`, `t9`); direction of the lowest-FID polygon bounding the line; rows = left set ×
  right set, polygons the line lies inside on both sides (`t4`, `t10`). Only LEFT_FID/RIGHT_FID.
- Feature To Polygon: outer ring starts after the top-right vertex. Lone hole: after its lowest
  (then leftmost) vertex; notches in the outer ring (`notch_*` in the session log: top-right/
  bottom-right notches move the entry to the hole's top-right vertex) are explained by an upward
  monotone-partition sweep (de Berg, mirrored) plus a walk round the outer ring from its start.
  Dense multi-hole faces (`hh`, `tri`) still differ in order/start. Labels: first label point
  (by FID) strictly inside; a label on the linework becomes a vertex (and in `b1` adds a second
  vertex 0.004 m on); NO_ATTRIBUTES ignores labels; without labels polygon inputs give FID_<name>
  -1 plus their fields as nulls, line inputs nothing (shapefile `Id` then).
- Feature To Line: lines split at crossings, at other lines' ends/joins and at their own ends;
  coincident stretches take the first polygon's direction, else the first line's (`dir_*`, `pl_*`);
  rows = product over inputs of the features there (−1 when none); NO_ATTRIBUTES gives one row per
  stretch with `Id`. Polygon rings: a lone ring keeps its start; crossed rings are sometimes split
  once more at a non-node (`rs*` at the vertex after the top-right, `d1` and `rp` FID 1 at the
  original start, `vp`/`grid`/other `rp` rings not at all) — unexplained.
- An arcpy-session pitfall: the session is shared; a variable like `sr` can be changed by another
  user between calls (some early probes, e.g. `L1`, `lab`, `t*`, ended up in RD New).
