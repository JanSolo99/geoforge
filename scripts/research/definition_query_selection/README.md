# Generate Definition Query From Selection — deferred

Public ArcGIS Pro 3.7 probe.py: a file path fails000732; a standalone feature layer
without selection fails000346; a selected standalone feature layer succeeds and
returns the same layer. The tool updates that layer's definition-query state; it
does not write a dataset or just return SQL. Although the public documentation
requires a map/scene layer, this control succeeds for MakeFeatureLayer outside a map.

The shared unigis-tools Context exposes dataset access, but no selection/highlight
identity or mutable layer definition queries. The CLI opens files, losing this
state. Implementing only SQL from all input rows would silently change the tool's
meaning. Defer until those host capabilities and layer-result comparisons exist.
No SDK used; no dataset edits, map session or coordinator launched. Source unchanged.

Reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/generate-definition-query-from-selection.html
