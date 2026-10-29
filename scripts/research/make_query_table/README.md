# Make Query Table — deferred

Public reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/make-query-table.html

2026-10-30: `probe.py` run through the public ArcGIS bridge; results saved in
`results.json`. Existing FileGDB `points` produces a named FeatureLayer (3 rows),
an OBJECTID-only projection with a predicate produces a TableView (2 rows),
and a false predicate produces an empty FeatureLayer. No persistent output is
requested; the objects live in the ArcGIS process.

Registry Context has no session-scoped query view store/handle; Outcome supports
materialized features/tables/reports only. A copied output table would lose the
view lifecycle, query identity, key selection semantics and qualified source
fields, so it is not a compatible default implementation. Requires shared host
view infrastructure plus SQL join/query evaluation; defer within queue time box.
