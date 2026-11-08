# Export XML Workspace Document probe (2026-11-08, ArcGIS Pro 3.7)

SCHEMA_ONLY export of a file gdb with one point feature class (UTM 32N, one LONG field) gives
~10 KB of XML (`esri:Workspace`, schema 10.8). Each DataElement embeds an escaped
`<Metadata>` XmlDoc with creation date/time and a lineage `Process` entry per geoprocessing
call (tool source path, date, time, full parameter echo with the WKT). Add the full
spatial-reference XML (WKID, grid, tolerances), DSID, GeometryDef and index definitions.
The lineage/timestamps make byte or even structural comparison depend on history we do not
record, so a faithful reproduction needs a metadata/lineage subsystem first.
