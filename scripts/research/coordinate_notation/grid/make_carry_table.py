"""ArcGIS bridge: create an explicitly typed nonspatial DBF carry fixture."""
import csv
import json

src = '/mnt/media/ArcGIS_Linux/scripts/research/coordinate_notation/grid/carry_precision_all_valid.csv'
target = '/mnt/media/ArcGIS_Linux/testdata/geoproc/grid_carry_precision.dbf'
table = 'memory/carry_fixture'
if arcpy.Exists(table): arcpy.management.Delete(table)
arcpy.management.CreateTable('memory', 'carry_fixture')
arcpy.management.AddField(table, 'ID', 'LONG')
arcpy.management.AddField(table, 'COORD', 'TEXT', field_length=255)
with open(P(src), newline='') as f, arcpy.da.InsertCursor(table, ['ID', 'COORD']) as cursor:
    for row in csv.DictReader(f): cursor.insertRow((int(row['ID']), row['COORD']))
arcpy.env.overwriteOutput = True
arcpy.management.CopyRows(table, P(target))
print(json.dumps({'path': target, 'fields': [(f.name, f.type) for f in arcpy.ListFields(P(target))],
                  'count': int(arcpy.management.GetCount(P(target)).getOutput(0))}))
