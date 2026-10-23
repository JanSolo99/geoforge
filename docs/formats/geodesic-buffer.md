# Geodesic Buffer (ArcGIS Pro 3.7, geographic input, distance in a linear unit)

Observed through the bridge; implemented in `crates/unigis-geometry/src/geodesic.rs`, checked on 25 cases
(`testdata/arcgis-reference/geodesic_buffers.json`, `crates/unigis-geometry/tests/geodesic_buffer.rs`).

- A plain number, or a distance in degrees, is a planar buffer in degrees (the existing planar rule with the
  geographic deviation). Only a linear unit gives the geodesic buffer. `BUFF_DIST` keeps the distance as given.
- **Points**: the geodesic circle on the ellipsoid. Vertices at azimuth `360 k / n` from north, clockwise, `n` from the
  planar rule `ceil(2π / (2 acos(1 - dev / r)))` (at least 36) with a deviation `dev` that steps with the distance `r`:
  0.1 m up to 1000 m, 1 m up to 10 000 m, 5 m up to 100 000 m, 10 m beyond (tested at 999/1000/1001 etc.). 223 steps at 1 km.
- **Lines and polygons** are built from the planar pieces (a rectangle per segment, an arc where a bend opens, half-discs
  at open ends, FLAT ends as the round buffer minus the exact half-discs) with each offset point found by the direct
  geodesic problem along the geodesic azimuths (`az ± 90°`). Their arcs do not use the point rule:
  - `dev` tiers: 0.1 m up to 1000 m, 1 m up to 10 000 m, 5 m up to 50 000 m, 100 m beyond.
  - The arc's vertices sit on a global azimuth grid: the step is the largest `10° / 2^k` whose chords stay within `dev`
    (`2 acos(1 - dev / r)`); the grid runs between the first and last multiple of 10° inside the arc; the stretches from
    the arc's ends to those anchors are divided by the planar rule for line arcs and into steps no longer than the grid's
    for polygon arcs (corners of rings).
  - Beyond 50 km ArcGIS's line arcs show a different phase (points at `1.25° + 2.5° k` and extra vertices that drift near
    the 10° anchors); the shape is matched only to the 100 m deviation.
  - ArcGIS leaves micro-vertices (under 0.1 m) at a ring's seam and resolves the inside of a sharp bend with extra vertices
    (0.8 m at a 65°N bend); these are not reproduced.
- Buffers over the antimeridian or a pole are refused (the longitude/latitude pieces stop being simple polygons).
