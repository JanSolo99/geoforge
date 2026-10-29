"""Independent GARS lattice holdout and fine GEOREF DMS carry controls."""
import csv
import json
import random

letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'
rng = random.Random(2947)
values = ['PKAA' + value + '100000000' for value in [
    '049998000', '049998300', '049998333', '049998334', '049998400',
    '599000000', '599900000', '599990000', '599995000', '599998000',
    '599998333', '599998334', '599999000', '599999900', '599999999' ]]
src = '/tmp/unigis-grid-georef-carry.csv'
with open(P(src), 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['ID', 'COORD']); w.writerows(enumerate(values, 1))
for fmt in ['DD_NUMERIC', 'DDM_1', 'DMS_1', 'GARS']:
    out = 'memory/georef_carry_' + fmt
    arcpy.management.ConvertCoordinateNotation(P(src), out, 'COORD', None, 'GEOREF', fmt, '', arcpy.SpatialReference(4326))
    fields = ['COORD'] + ({'DD_NUMERIC': ['DDLon', 'DDLat'], 'DDM_1': ['DDMLatLon'], 'DMS_1': ['DMSLatLon'], 'GARS': ['GARS']}[fmt])
    with arcpy.da.SearchCursor(out, fields) as c:
        print(fmt, json.dumps(list(c)))
