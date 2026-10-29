"""ArcGIS bridge: testdata/geoproc/ctf_text.dbf, text and numeric dates for Convert Temporal Field.

Run: uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/research/convert_time_field/make_inputs.py
"""
t = 'memory/ctf_in'
if arcpy.Exists(t): arcpy.management.Delete(t)
arcpy.management.CreateTable('memory', 'ctf_in')
for n, k in [('ID', 'LONG'), ('S1', 'TEXT'), ('S2', 'TEXT'), ('S3', 'TEXT'), ('S4', 'TEXT'), ('N1', 'LONG')]:
    arcpy.management.AddField(t, n, k)
rows = [(1, '20240229', '02/29/2024', '2024-02-29 13:45:10', '29 February 2024', 20240229),
        (2, '19991231', '12/31/1999', '1999-12-31 00:00:00', '31 December 1999', 19991231),
        (3, '2001011', '1/1/2001', '2001-01-01 7:05:00', '1 January 2001', 2001011),
        (4, 'notadate', '13/40/2020', '2020-02-30 00:00:00', 'x', 20200230),
        (5, None, None, None, None, None),
        (6, '20200101', '01/02/2020', '2020-01-02 23:59:59', '2 Jan 2020', 20200101)]
with arcpy.da.InsertCursor(t, ['ID', 'S1', 'S2', 'S3', 'S4', 'N1']) as c:
    for r in rows: c.insertRow(r)
arcpy.env.overwriteOutput = True
arcpy.management.CopyRows(t, P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ctf_text.dbf'))
print([f.name for f in arcpy.ListFields(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ctf_text.dbf'))])
