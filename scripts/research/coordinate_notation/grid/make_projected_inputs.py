"""ArcGIS bridge: typed nonspatial DBF fixtures of UTM_BANDS/UTM_ZONES/MGRS/USNG strings.

The recipe CSVs hold ArcGIS's own output strings for the 39 lonlat_grid points and 150 random
points (seed 20261029), then hand-written variants and invalid strings. A DBF avoids the
virtual COORD_X/COORD_Y fields ArcGIS adds when it reads coordinate strings from a CSV.
"""
import csv
import json

grid = '/mnt/media/ArcGIS_Linux/scripts/research/coordinate_notation/grid'
for fmt in ['mgrs', 'usng', 'utm_bands', 'utm_zones']:
    src = f'{grid}/projected_input_{fmt}.csv'
    target = f'/mnt/media/ArcGIS_Linux/testdata/geoproc/grid_input_{fmt}.dbf'
    table = 'memory/projected_fixture'
    if arcpy.Exists(table): arcpy.management.Delete(table)
    arcpy.management.CreateTable('memory', 'projected_fixture')
    arcpy.management.AddField(table, 'ID', 'LONG')
    arcpy.management.AddField(table, 'COORD', 'TEXT', field_length=255)
    with open(P(src), newline='') as f, arcpy.da.InsertCursor(table, ['ID', 'COORD']) as cursor:
        for row in csv.DictReader(f): cursor.insertRow((int(row['ID']), row['COORD']))
    arcpy.env.overwriteOutput = True
    arcpy.management.CopyRows(table, P(target))
    print(json.dumps({'path': target, 'fields': [(f.name, f.type) for f in arcpy.ListFields(P(target))],
                      'count': int(arcpy.management.GetCount(P(target)).getOutput(0))}))
