# Eliminate — deferred

Public ArcGIS Pro3.7 controls: file input fails000229; an unselected feature layer
fails000346. Selecting PARCEL_ID1 succeeds but that separated-parcel fixture has no
shared boundary, so all36 polygons remain. The adjacent-neighbor control instead
selects a2m sliver between10m and20m neighbors: AREA eliminates it into the large
unselected neighbor, leaving KEY1 area100 and KEY3 area220.

The native geometry overlay can union polygons, but the shared Context/CLI has no
selected-row identity while retaining the complete unselected source for merge
destinations and output. Passing only selected features loses the neighbors;
passing the file has no selection. A selected-rows-only or all-polygons merge would
change this tool's meaning. Defer until selection-aware host support exists.

probes/results use disposable outputs only, no SDK or source dataset edits.
https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/eliminate.html
