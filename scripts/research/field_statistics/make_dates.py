"""ArcGIS bridge: testdata/geoproc/dates.dbf, the Field Statistics To Table DATE fixture.

25 rows: ID, GRP (= ID % 3), D (dates with duplicates, two nulls, 1900..2099), E (2000-01-01
plus 37*ID days, null every fifth row).
Run: uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/research/field_statistics/make_dates.py
"""
import datetime

tgt = P('/mnt/media/ArcGIS_Linux/testdata/geoproc/dates.dbf')
t = 'memory/dates_fx'
if arcpy.Exists(t): arcpy.management.Delete(t)
arcpy.management.CreateTable('memory', 'dates_fx')
for name, kind in [('ID', 'LONG'), ('GRP', 'SHORT'), ('D', 'DATE'), ('E', 'DATE')]:
    arcpy.management.AddField(t, name, kind)
D = datetime.datetime
ds = [D(2024, 2, 29), D(1999, 12, 31), D(2001, 1, 1), D(2024, 2, 29), None, D(1970, 1, 1), D(2020, 6, 15),
      D(2020, 6, 16), D(1900, 1, 1), D(2099, 12, 31), D(2010, 5, 5), D(2010, 5, 5), D(2010, 5, 5), None,
      D(1985, 7, 4), D(2000, 2, 29), D(2015, 3, 1), D(2015, 3, 2), D(1950, 8, 8), D(2030, 1, 1),
      D(2005, 10, 10), D(1999, 1, 1), D(2012, 12, 12), D(1960, 6, 6), D(2023, 11, 30)]
with arcpy.da.InsertCursor(t, ['ID', 'GRP', 'D', 'E']) as c:
    for i, d in enumerate(ds, 1):
        c.insertRow((i, i % 3, d, None if i % 5 == 0 else D(2000, 1, 1) + datetime.timedelta(days=37 * i)))
arcpy.env.overwriteOutput = True
arcpy.management.CopyRows(t, tgt)
print([(f.name, f.type) for f in arcpy.ListFields(tgt)])
