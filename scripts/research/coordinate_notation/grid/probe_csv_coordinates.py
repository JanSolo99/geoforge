"""Inspect automatic text-table coordinate fields separately from notation formatting."""
import csv
import json

src = '/mnt/media/ArcGIS_Linux/scripts/research/coordinate_notation/grid/carry_precision_all_valid.csv'
with open(P(src), newline='') as f: rows = list(csv.reader(f))
for label, data in [('all_valid', rows), ('with_invalid', rows + [[149, 'junk']])]:
    path = '/tmp/unigis-carry-csv-' + label + '.csv'
    with open(P(path), 'w', newline='') as f: csv.writer(f).writerows(data)
    print(label, 'input', [(f.name, f.type) for f in arcpy.ListFields(P(path))])
    out = 'memory/carry_csv_' + label
    arcpy.management.ConvertCoordinateNotation(P(path), out, 'COORD', None, 'GEOREF', 'DMS_1', '', arcpy.SpatialReference(4326))
    print(label, 'output', [(f.name, f.type) for f in arcpy.ListFields(out)])
    names = [f.name for f in arcpy.ListFields(out)]
    with arcpy.da.SearchCursor(out, [n for n in names if n not in ('OBJECTID', 'Shape')]) as cursor:
        print(label, 'first', json.dumps(next(cursor), default=str))
