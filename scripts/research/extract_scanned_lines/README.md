# Extract Scanned Lines — deferred (2026-10-30)

`arcpy.conversion.ExtractScannedLines(in_raster, foreground_value, out_centerlines,
intersection_solution=GEOMETRICAL, line_width=20, noise_level=65, compression=0.025,
smoothing=True, smoothing_weight=3, corners=False, max_corner_angle=135, hole_size=0,
close_gaps=False, gap_tolerance=5, gap_search_angle=100, out_polygons)`; output fields `Width`.
`compression` must be 0.001–50 (ERROR 000854). Probe: `probe.py` (ArcGIS Pro 3.7).

- Axis-aligned cross (5 px and 3 px bars): four centerlines split at the junction (39.5, 29.5),
  ending on the pixel edges; Width 4.8 / 4.9 for the 5 px bar (measured, not the pixel count), 3.0
  for the 3 px bar.
- Slanted 4 px line touching a 3 px ring: without smoothing the centerline is a half-pixel
  staircase (40.5 20, 40.5 19, 41.5 18, 41.5 17.5, ...); the GEOMETRICAL intersection solution moves
  junctions to computed points (37.153558225385503 25.793086239969782); smoothing replaces every
  vertex (3 80, 4.9934196010751357 78.19693236807673, ...); compression 0.001 adds vertices such
  as 40.3456242505994 20.1543757494006.

Why deferred: the output is ArcScan's tracing — skeleton walk, junction solution, noise and width
estimates, smoothing and compression — none of it documented. Matching it to 0.001 m needs the
algorithm reverse-engineered from many more black-box probes than the queue's time box allows.

# Extract Scanned Polygons — deferred with it (same ArcScan engine)

`ExtractScannedPolygons(in_raster, foreground_value, out_polygons, compression=0.025,
smoothing=True, smoothing_weight=3, hole_size=0)`; output field `Id` (0).

- Default (smoothing on): outlines with irregular vertices (10.425681043015905 49.574384238792405,
  10.226480350517535 49.315606015643326, ...) — undocumented smoothing.
- `smoothing=False`, default compression: exact pixel-edge outlines; 8-connected regions (two
  squares touching at a corner are one polygon whose ring touches itself); holes kept; collinear
  vertices dropped; outer ring clockwise from the top-left corner of the top row, holes
  counter-clockwise; features in top-down scan order. `hole_size=n` drops holes of area < n pixels.
- `compression=0.5` already turns a radius-12 disk into an octagon; `compression=1` drops a 3x3+3x3
  corner-touching pair entirely — the simplification and small-region rule are not identified.

A future attempt could start from the unsmoothed case (Raster To Polygon-like, 8-connected) but
the default output needs the smoothing reverse-engineered.
