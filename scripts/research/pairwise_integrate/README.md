# Pairwise Integrate — deferred

Public reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/pairwise-integrate.html

33 disposable ArcGIS Pro 3.7 controls:16 forward/reverse feature controls,
14 cluster controls and3 default-tolerance controls. Recipes and raw results
are adjacent. No user fixtures were modified; parallel processing was disabled.

Crossing lines gain vertices, including self crossings. A vertex0.6 m above
an edge at tolerance1 moves to the midpoint0.3 m and bends that edge; short
collapsed lines are removed. Nearby parallel lines move to their midpoint.
Existing LineNodes only handles intersections between paths, not this complete
clustering/collapse behavior; the repair helper works per geometry.

Clustering is not a simple distance-threshold connected component followed by
its centroid. A two-point2 m separation merges at1 m tolerance. Points at
0,0,0.6 m yield0.1 m for all rows, rather than the row centroid0.2 m or unique
coordinate centroid0.3 m. A ten-point0.6 m chain yields two cluster positions
(5.4 m and2.4 m), with reordered output coordinates relative to source order.
At the DEFAULT0.001 m dataset tolerance, duplicate points0,0,0.0006 yield
0.0001 m; a0.002 m pair merges. The ten-point default-scale chain instead
collapses entirely to0.0027 m. All coordinates translated to UTM32633.

A complete compatible cluster engine and multi-input in-place host capability
are absent. Defer instead of exposing a crossing-only or fitted approximation.
No runtime tool or active reference cases are claimed. Reopen only on request.

Reproduce when needed:

    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/pairwise_integrate/probe.py
    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/pairwise_integrate/probe_clusters.py
    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/pairwise_integrate/probe_default.py
