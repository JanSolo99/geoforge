# ArcGIS Pro polygon label point (`Polygon.labelPoint` / Feature To Point INSIDE)

Black-box reconstruction. I only used ArcGIS input/output (oracle files plus about 20 focused
`arcpy` probe batches in `lp/p*.py`). Implementation: `labelpoint.py`, `label_point(rings)`.

## Accuracy

| set | match within 1e-3 |
|---|---|
| oracle `rp_res.json` labelPoint (300) | **296 / 300** (stars 99/100, histograms 60/60, rotated 40/40, holes 28/30, convex 40/40, two-part 29/30) |
| oracle Feature To Point INSIDE (300) | **298 / 300** |
| 12 hand shapes (ArcGIS-simplified rings, `lp/hand_rings.json`) | **12 / 12** |
| fresh validation set, in-memory arcpy (`lp/val*.py`: 120 stars, 60 rectilinear incl. rotated, 60 with holes, 60 two-part) | **300 / 300** |

Failures:
- **215, 225**: exact distance ties between the 1/4 and 3/4 candidates (rectangles with a
  hole). Feature To Point and in-memory `labelPoint` agree with us (3/4). Only `labelPoint`
  read back from the shapefile gives 1/4, probably because of coordinate storage and
  rounding. This is noise in the oracle.
- **4, 298**: the self-intersecting input was resolved into an outer ring plus a hole that
  touches it at 2–3 vertices. ArcGIS's answer is close to our reference centroid but not
  equal, so ArcGIS evidently models these degenerate touching holes in some other way.
  Not resolved.

## The rule (precise enough to port)

Input: the polygon as ArcGIS stores it after simplification. Outer rings run clockwise
(negative shoelace area). Holes run counter-clockwise. Self-intersections are already
resolved. (Shapes that are not simple must be simplified first. For example, 'z' becomes
two triangles/trapezoids.)

1. **Part.** Group the rings into parts. A part is an outer ring plus the holes that lie
   inside it or share a vertex with it. Keep only the part with the largest net area; on
   equal area the first one wins. Everything below uses only this part, including its
   envelope and its boundary for distances.
2. **Reference point C.** Take the area centroid of the part's outer ring, plus only the
   holes that *share a vertex* with the outer ring (signed areas, so those holes subtract).
   Holes strictly inside the outer ring are **ignored** for C. Example: in a rectangle with
   an interior hole, C is the rectangle's center.
3. **Line candidates.** For f in (1/2, 3/4, 1/4), take x = xmin + f·(xmax − xmin) of the
   part's envelope. Intersect the vertical line X = x with all ring edges, using the
   half-open rule: an edge counts if `(x1 > x) != (x2 > x)`. Sort the crossing y values.
   The candidate is the midpoint of the **lowest** interval, (y0 + y1)/2. Skip a line that
   has no crossings. Only these three lines are used; finer fractions never appeared.
4. **Ranking.** Sort the line candidates by Euclidean distance to C (C may lie outside the
   polygon). Use a stable sort with initial order (1/2, 3/4, 1/4), so on exact ties 3/4
   comes before 1/4.
5. **Weights.** C is a candidate only if it is inside the part (even-odd test), with
   weight **6**. The line candidates get weights **5, 4, 3** by their rank in step 4. When
   C is outside, the weights are still 5, 4, 3 (they do not shift up).
6. **Score.** score = (Euclidean distance from the candidate to the part boundary,
   including holes) × weight. Return the candidate with the highest score, testing in the
   order C, then rank 0, 1, 2. A later candidate replaces the current one only on a strict
   `>`.

Probes pinned down the weights. A side slit pushed toward C gives a threshold of exactly
d_C = 2/3·d(quarter). The notch probes gave a C/quarter ratio between 0.662 and 0.669. A
slanted slit across the 1/2 line gives d_C = 5/6·d(1/2). Comb shapes, with C pushed
left or right by a spike, flip between the 1/4 and 3/4 bars at e_near = 0.75·e_far, and
the flip happens exactly when C.x crosses the envelope center. Between v1/2 and a
quarter, the closer one gets 5 and the other 4, so the switch ratio is 4/5 or 5/4
depending on which is closer to C. The ranking is by 2-D distance; ranking by x-distance
only scored 277 of 289 matchable cases, against 289 of 289 for 2-D distance.

## Notes for the Rust port

- Use Euclidean point-to-segment distance against every ring of the chosen part.
- No snapping is needed. The in-memory arcpy geometry without a spatial reference adds
  +1/8192 (2^-13) to the outputs. The shapefile output does not. Compare with a tolerance
  of 1e-3.
- If there are no candidates (degenerate input), return C.
- The current `inside_point` in `crates/unigis-geometry/src/bounding.rs` (centroid if
  inside, else a horizontal scan) should be replaced by this rule. Those hypotheses
  (horizontal scans, widest slab, plain centroid-if-inside, and polylabel) are ruled out.

## Verification in this repository
`inside_point` in `crates/unigis-geometry/src/bounding.rs` implements this rule; the
reference case `f2p_label_polys_inside` (298 random polygons, `testdata/geoproc/label_polys.shp`)
and the older Feature To Point cases match ArcGIS. The two degenerate inputs (4 and 298 of the
original set) are left out.
