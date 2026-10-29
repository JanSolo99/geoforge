"""Independent GARS lattice holdout and fine GEOREF DMS carry controls."""
import csv
import json
import random

letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'
rng = random.Random(2947)
values = []
for _ in range(240):
    band, row = rng.randrange(1, 721), rng.randrange(360)
    quadrant, key = rng.randrange(1, 5), rng.randrange(1, 10)
    values.append(f'{band:03}{letters[row//24]}{letters[row%24]}{quadrant}{key}')
src = '/tmp/unigis-grid-gars-holdout.csv'
with open(P(src), 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['ID', 'COORD']); w.writerows(enumerate(values, 1))
out = 'memory/gars_holdout'
r = arcpy.management.ConvertCoordinateNotation(P(src), out, 'COORD', None, 'GARS', 'GARS', '', arcpy.SpatialReference(4326))
with arcpy.da.SearchCursor(out, ['COORD', 'GARS']) as c:
    print('GARS_HOLDOUT', json.dumps(list(c)))
values = ['PKAA' + value + '100000000' for value in [
    '049999000', '049999900', '049999990', '049999991', '049999995',
    '049999996', '049999997', '049999998', '049999999', '050000000',
    '050000001', '050000002', '050000005', '050000010']]
src = '/tmp/unigis-grid-georef-carry.csv'
with open(P(src), 'w', newline='') as f:
    w = csv.writer(f); w.writerow(['ID', 'COORD']); w.writerows(enumerate(values, 1))
for fmt in ['DD_NUMERIC', 'DDM_1', 'DMS_1', 'GARS']:
    out = 'memory/georef_carry_' + fmt
    arcpy.management.ConvertCoordinateNotation(P(src), out, 'COORD', None, 'GEOREF', fmt, '', arcpy.SpatialReference(4326))
    fields = ['COORD'] + ({'DD_NUMERIC': ['DDLon', 'DDLat'], 'DDM_1': ['DDMLatLon'], 'DMS_1': ['DMSLatLon'], 'GARS': ['GARS']}[fmt])
    with arcpy.da.SearchCursor(out, fields) as c:
        print(fmt, json.dumps(list(c)))
