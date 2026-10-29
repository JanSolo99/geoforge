# Remove Overlap (multiple): deferred

The public ArcGIS Pro 3.7 probe in probe.py saves18 method/scenario combinations
in probe_results.json. Six inputs cover equal rectangles, unequal containment,
oblique boundaries, nested polygons, triple overlap and disjoint inputs. Only
public arcpy input construction and black-box GP calls were used.

The default CENTER_LINE is not equivalent to the existing point-centroid
Voronoi primitive. With rectangles A=[0,20]x[0,20] and B=[10,30]x[0,20], CENTER_LINE
uses the boundary [(20,0),(15,5),(15,15),(20,20)]. Output areas are325 and275,
while THIESSEN uses a straight x=15 cut with areas300/300. GRID returns a different
partition (225/375). Coordinates are translated by(500000,5000000) in the files.
Containment can retain an output row with null geometry.

The native engine has overlay and point Voronoi primitives, but no medial-axis/
straight-skeleton boundary partitioner. Implementing only THIESSEN or fitting a
rectangle special case would leave the default unsupported. Deferred under the
queue time box; no runtime stub or completed-tool claim was added. A future
implementation must explain oblique, nested and triple controls as well as the
default rectangle, including layer order, attribute merging and null outputs.

Public reference:
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/remove-overlap-multiple.html

Reproduce only when needed:

    uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/remove_overlap/probe.py

The result is written to /tmp/unigis-rom-probe.json. ArcGIS outputs already saved;
do not repeat these external calls merely to recover context.
