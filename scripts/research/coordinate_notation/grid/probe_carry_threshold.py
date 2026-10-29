"""Independent GARS lattice holdout and fine GEOREF DMS carry controls."""
import csv
import json
import random

letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'
rng = random.Random(2947)
values = ['PKAA' + value + '100000000' for value in [
    '049998999', '049999000', '049999001', '599939999', '599940000', '599940001' ]]
src = '/tmp/unigis-grid-georef-carry.csv'
with open(P(src), 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['ID', 'COORD']); w.writerows(enumerate(values, 1))
for fmt in ['DD_NUMERIC', 'DDM_1', 'DMS_1', 'GARS']:
    out = 'memory/georef_carry_' + fmt
    arcpy.management.ConvertCoordinateNotation(P(src), out, 'COORD', None, 'GEOREF', fmt, '', arcpy.SpatialReference(4326))
    fields = ['COORD'] + ({'DD_NUMERIC': ['DDLon', 'DDLat'], 'DDM_1': ['DDMLatLon'], 'DMS_1': ['DMSLatLon'], 'GARS': ['GARS']}[fmt])
    with arcpy.da.SearchCursor(out, fields) as c:
        print(fmt, json.dumps(list(c)))
