# Raster To Polygon SIMPLIFY: reverse-engineering notes

> Status 2026-10-15: solved for blocky rasters (104 of 122 test rasters exact, vertex by vertex within 1e-3); smooth curves (kept-corner selection) not yet. Research scripts: `scripts/research/raster-to-polygon-simplify/` (`simplify.py`, `score.py`).


All coordinates are in cell units: cellsize 1, origin (0,0) at the lower left. ArcGIS results
scale exactly with cellsize (tri7 at cs=30 is tri7 at cs=1 times 30).
Probe pipeline: `cases.py` defines the rasters, `gen.py` writes them to tif/, `probe_arc.py` runs
RasterToPolygon SIMPLIFY in ArcGIS and dumps JSON to out/<case>.json, and `show.py`/`showr.py`
print the results (showr also prints rational forms).

## Established facts
1. **Topology.** Boundaries are split into arcs between nodes. A node is a corner where 3 or more
   regions meet, counting the outside of the raster as a region. Node points are fixed exactly. A ring
   with no node starts at the bottom-right corner of its lowest row (the rightmost corner with the
   smallest y), and that start vertex is also fixed. Edges on the raster border stay exact.
2. **Corner displacement `a`.** Every interior corner of an arc moves diagonally by `a` in x and y,
   toward the inside of the turn, i.e. toward the centre of the single cell that differs ("minority"
   quadrant). Convex corners move inward and concave corners move outward. A single `a` applies to
   the whole arc; asymmetric notches confirm that both sides use the same `a`.
   `a` always lies on the grid `a_k = 0.49 - k*0.29/32`, k = 0..32 (0.49 down to 0.2), which is why
   coordinates have denominators that divide 3200. Values seen: n1 notch k = 0..9 depending on arm
   length, n2 (2-high, 1-wide notch) k=23, h3 k=18, h4 k=15, bump2 k=20, ramp2-6 k=26,
   ramp3_run2 k=17, ramp3_run3 k=12, long n1 notch (L>=25) k=31, tri3 k=17.
3. **Simplification of the displaced polyline.** It works like Douglas-Peucker: split at the point
   farthest from the chord. When two **adjacent** points tie for the maximum distance (e.g. the
   flat top of a notch, parallel to the chord), both are kept and then that short middle segment is
   collapsed: the two vertices are replaced by the intersection of the line
   (previous kept vertex -> first tie point) with the line (second tie point -> next kept vertex).
   This produces the overshooting apexes (e.g. a 1-wide, 2-high notch has its apex at 7.2758 > 7).
   A tie between non-adjacent points (ramp2) keeps both points and does not collapse.
4. **Choice of `a`.** This part is not fully settled. The best rule so far:
   - validity P(a): every point of the output polyline lies within distance `a` of the original
     cell-edge boundary (d_ro <= a, directed from output to original);
   - among valid candidates, pick the one with the fewest vertices, then the smallest deviation.
     This explains n1_L20 (k=0, 1 vertex) versus L>=25 (k=31, 3 vertices). The transition at L24/L25
     is exactly where d_ro(k=0) crosses 0.49.
   - The chosen k is always the grid point on the large-`a` side of the continuous minimiser. For
     example, with n1_L5 the d_ro minimum is at k=4, but k=4 is invalid (0.4548 > a=0.4538).
   - Open question: which vertices get inserted. n1_25_15 keeps c1 even though its DP distance is
     0.0004, so insertion is driven by the deviation check rather than a DP tolerance. The current
     attempt (m2.py) splits a segment when its deviation from the original exceeds `a`, or when a
     displaced point is farther than T from the chord (T unknown; 0.5 or none).
5. **No smoothing at all (exact corners).** step2/3/4/6, rectangles with sides >= 2 (except 2x2),
   and squares 3x3+ come out with exact corners. A fallback is the likely explanation: when no `a`
   is valid, the original corners are kept, i.e. NO_SIMPLIFY geometry for that arc. A step of
   height 1 (step1) becomes one straight line from node to node.
6. Single cell, 1x8 line and 2x2 block (closed rings with a single fixed start vertex) become odd
   triangles. Not modelled yet.

## Status
- Profile family (horizontal boundary with notches, steps and ramps between two border nodes):
  hyp.py with fixed k reproduces n1 (all L), n2, h3, h4, bump2, ramps and steps exactly when `a`
  is given. Choosing `a` automatically is still being validated (t16.py / m2.py).
- Not yet matching: notch_two (two notches on one arc), and n1_8_25 before the insertion rule.

## Update 1: profile family solved (73/73 profile cases match exactly, t16.py + m2.py)
The rule that works for every profile case (notches of any height and width, asymmetric arms,
two notches, steps, ramps, bumps):
- Candidates are a_k = 0.49 - k*0.29/32 for k = 0..32, plus the exact candidate (original corners,
  deviation 0).
- For each candidate: displace the corners by a_k, then simplify recursively. A segment D_i-D_j is
  split when its maximum distance to the original cell-edge chain is greater than a_k (no fixed
  DP tolerance is needed). The split point is the displaced point farthest from the chord. On a
  tie, the first and last of the tied points are both kept.
- Collapse: two consecutive kept points that are adjacent in the arc (i, i+1) and whose original
  corners turn the same way (both convex or both concave, i.e. the two ends of a "U" run) are
  replaced by the intersection of line(prev kept -> D_i) with line(D_{i+1} -> next kept).
- The candidate is valid if the final polyline stays within a_k of the original chain.
- Choose the candidate with the fewest vertices, then the smallest deviation. The exact candidate
  takes part in this, which is why steps of height >= 2 and big rectangles stay exact.

## Update 2: general arcs (simplify.py)
- The DP split point is the ORIGINAL corner farthest from the chord between the ORIGINAL corners
  (not the displaced points). The split decision is still "the displaced segment deviates more than
  a from the original sub-chain". On a tie, the first and last tied corners are both kept.
- Collapse rule: ANY two consecutive kept vertices whose original corners turn the same way are
  replaced by intersect(line(prev kept D, D_i), line(D_j, next kept D)), scanning left to right
  in non-overlapping pairs. This explains tri3/tri4/... (two concave corners around a convex one
  collapse into 1+2a on the diagonal).
- ArcGIS writes holes: re-dumped all outputs with shape.JSON (an earlier dump lost the inner rings).
- Score now: all profiles, tri*, squares, square_hole, notch_two, bump2, ramps and steps PASS.
  Closed node-less rings (single cell, 2x2, line8, circles) FAIL.

## Update 3 (simplify.py = simplify_v4.py logic)
- Collapse sign is the turn of the KEPT displaced polyline at each kept vertex (D[K_{m-1}], D[K_m],
  D[K_{m+1}]), not the original corner's turn. Scan from the start: if K_m and K_{m+1} turn the same
  way, X = intersect(line(last output vertex, D[K_m]), line(D[K_{m+1}], D[K_{m+2}])). If X lies
  within a of the original boundary, output X and skip both. Otherwise clamp: move from D[K_m]
  toward X and stop at the first point whose distance to the original exceeds a; output that point
  and advance by one only, so K_{m+1} can pair with the next vertex. This clamp is what makes a
  single cell a triangle and gives the -a coordinates.
- Closed rings without nodes: the start vertex (lowest y, then highest x) stays fixed, and the ring
  is simplified clockwise. A hole ring (counter-clockwise) reuses the clockwise result, reversed.
  DP on a degenerate chord (start == end) always splits at the farthest corner from the start.
- Score: single, iso_cell, block2, line8, corner1 PASS. landuse_u8 (13 polys) and zones_25m PASS
  exactly. circ5/10/20 FAIL: the kept set differs. ArcGIS keeps pairs of corners that are the ends
  of runs ({0,1,4,5,7,8,9,10,12,13,14,15,17,18,19} for circ5, k=5). An alternative "DP over run
  midpoints" (simplify_v5_rundp.py) did not fix circles and broke bump2.

## Update 4
- **Arc direction:** each arc is processed with the HIGHER-valued region on its LEFT (the outside
  of the raster counts as +inf, so outer boundaries run clockwise). This matters because the DP
  tie-break (first) and the collapse scan both go from the arc start. With it, the zones circle's
  collapse/clamp output reproduces exactly once the kept set is forced.
- **Clamp detail:** slide in small steps from D[K_m] toward X and stop at the FIRST point whose
  distance to the original exceeds a (a doubling search missed thin crossings).
- **Open problem, the kept set on smooth curves (circles, the zones ellipse):** ArcGIS's kept
  vertices come in "pieces". Each output line passes through the displaced endpoints of a staircase
  piece that runs from a right-turn corner to a left-turn corner (relative to the arc direction),
  and consecutive piece-lines are intersected. The zones arc (cell units, reversed, k=9) needs
  kept = {0,1,2,7,10,13,16,17,22,25,28,30,31}. No DP or greedy variant tried (deviation of the
  displaced or original chain, tolerance 0.2-2, tie first/last/both) reproduces it. This looks like
  Potrace-style "minimum number of segments" over digital-straight pieces, which is not reproduced.

## Update 5
- **Open-arc direction:** an open arc starts at the node that comes last in raster order (smallest
  y, then largest x), the same rule as the closed-ring start. Closed rings run clockwise. This fixed
  the zones forced-kept test and tri4_r3 (the scan order matters for collapse and clamp).
- **DP tie rule:** TIE=first (keep only the first farthest corner) combined with
  CHORD=disp_orig (distance of the ORIGINAL corners to the chord between the DISPLACED endpoints
  D_i-D_j; at the top level these are the fixed nodes, so it equals the original chord there) fixes
  the digital-straight cases dss_2_5 and bend_13_0 and keeps every earlier pass. Under the
  displaced chord, tri3's second concave corner is chosen by the recursion, not by a tie.
- Realistic rasters (real_*, smooth class boundaries) still fail broadly. They are the main
  remaining gap.

## Update 6: final state of simplify.py (all defaults: TIE='both', CHORD='disp_orig', SPLIT='le', MODE='grid')

### Rule as implemented (cell units; the result is scaled by cellsize and shifted to the origin)
1. **Regions:** 4-connected regions of equal value, numbered in raster order of their first cell.
   NoData and the outside of the raster are "outside" (value +inf).
2. **Nodes:** grid corners where 3 or more distinct labels meet among the 4 surrounding cells
   (outside counts as a label), or where 2 labels meet in a checkerboard. A region ring is cut at
   its nodes into arcs. A ring with no node is one closed arc.
3. **Arc direction:** an open arc starts at the end node that is last in raster order (smallest y,
   then largest x). A closed arc starts at its corner with the smallest y, then the largest x, and
   runs clockwise. Each arc is simplified once, and both neighbouring polygons use that same result
   (reversed where needed), so shared edges stay identical.
4. **Corners:** the arc's unit-step corner list with collinear points removed. The end nodes, and
   the start of a closed arc, never move.
5. **Candidates:** a_k = 0.49 - k*0.29/32 for k = 0..32, plus "exact" (the original corners).
6. **For each a:** move every interior corner by (a*(d2x-d1x), a*(d2y-d1y)), where d1 and d2 are the
   unit directions of the incoming and outgoing runs. This is a diagonal move of a toward the
   inside of the turn. Call the moved points D.
7. **Recursive split on span (i, j)**, starting with (0, n-1):
   - stop if j <= i+1;
   - stop if max over the segment D_i-D_j of the distance to the original sub-chain orig[i..j] is
     <= a + 1e-7 (a degenerate chord, closed start == end, always splits);
   - otherwise take the ORIGINAL corner(s) m in (i, j) farthest from the line D_i-D_j. Keep the
     first and the last of the tied maxima (tolerance 1e-9) and recurse on the sub-spans.
8. **Collapse pass** over the kept list K (scan from the arc start, skipping the end points). The
   turn sign at K_m is cross(D[K_m]-D[K_{m-1}], D[K_{m+1}]-D[K_m]).
   If sign(K_m) == sign(K_{m+1}) and K_{m+1} is not the last vertex:
   X = intersect(line(last output vertex, D[K_m]), line(D[K_{m+1}], D[K_{m+2}])).
   If dist(X, original chain) <= a, output X and advance by 2. Otherwise output the clamp point:
   walk from D[K_m] toward X and stop at the first point whose distance to the original exceeds a.
   Advance by 1. Any other vertex is output as D[K_m].
9. **Validity:** the candidate is valid if the max distance from the output polyline to the original
   chain is <= a + 1e-7.
10. **Choice:** among valid candidates, the one with the fewest vertices, then the smallest
    deviation. The exact candidate wins whenever its vertex count is <= the best smoothed count, so
    steps of height >= 2 and rectangles stay exact.
11. **Rings:** the outer ring is the one with the largest area. ArcGIS lists holes (scorer ignores
    ring order).

### Accuracy (score.py; vertex-by-vertex, 1e-3 m, cyclic ring match), file score_final.txt
- 122 rasters: 104 match completely. 1136/1267 polygons are exact, and 6656/7902 ArcGIS vertices have
  a model vertex within 1e-3.
- Project rasters: landuse_u8 13/13 polygons exact, classes_u8 515/515, zones_25m 14/14, zones 4/7
  (the ellipse fails).
- Synthetic families all exact: notches of any height, width and arm length, two notches, bumps,
  ramps, steps, squares and rectangles, holes and islands, T-junctions, the single cell, the 2x2
  block, the 1x8 line, corner cases, tri* staircases, triaxis, and the 30 m cellsize case.

### Still failing (honest list)
- Smooth curves and long mixed staircases: circ5/10/20, the zones ellipse, arc_par, arc_par2,
  bend_11_13, bend_13_11, and the real_* rasters (classified DEM, smoothed noise blobs, concentric
  rings) at 0-40% of polygons. Pure random noise is 342/359.
  The cause is the kept set (step 7). ArcGIS keeps "pieces" whose lines run through displaced
  endpoints of digital-straight staircases, and my deviation-driven split keeps a different set.
  The collapse/clamp pass (step 8) reproduces ArcGIS exactly when given ArcGIS's kept set (checked
  on the zones ellipse: zsim.py with the forced set).
- **Borderline equalities:** the step-7/9 deviation always equals a at the displaced endpoints, so
  validity is decided by float ties. This affects dss_3_4 (ArcGIS k=6, model k=7), n1_L2,
  tri4_r3 and tri5_r2 (ArcGIS splits a span whose interior deviation is exactly a). The real
  criterion probably excludes the endpoints or uses a different distance (tested: original-corner
  DP tolerance T, edge-midpoint distance <= a, interior-only deviation, tie first/last, greedy
  forward. None works across all cases).
- **k-selection:** the true search looks like a bisection that returns the larger-a end of the
  bracket around the optimum (5 halvings of [0.2, 0.49] give the 0.29/32 grid). The model uses a
  grid argmin, which differs when the optimum falls between grid points (dss_3_4).

### Files
- simplify.py: simplify_polygons(raster_array, cellsize, origin, nodata=None) ->
  [{'value', 'rings'}]. Switches TIE/CHORD/SPLIT/MODE/TOL hold the experiment variants.
- score.py: `python3 score.py [case ...]` compares against out/<case>.json. ArcGIS JSON comes from
  probe_arc.py through the bridge; rasters are in tif/ and come from cases.py/gen.py (synthetic)
  and genreal.py (real_*). Project rasters are copied into tif/.
- Older model versions: simplify_v1..v7.py, simplify_v5_rundp.py. Profile-only experiments:
  hyp.py, m2.py, t*.py.
