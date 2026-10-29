"""Dense black-box carry thresholds across axes and hemispheres (ArcGIS bridge)."""
import csv
import json

letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'

def coord(x, xm, y, ym):
    a, b = x + 180, y + 90
    return f'{letters[a//15]}{letters[b//15]}{letters[a%15]}{letters[b%15]}{xm:09}{ym:09}'

configs = [('east0', 0, 45, 'x', False), ('east15', 15, 45, 'x', False),
           ('east151', 151, 45, 'x', False), ('west15', -16, 45, 'x', True),
           ('north45', 15, 45, 'y', False), ('south33', 15, -34, 'y', True)]
all_results = []
for fmt, start, stop, step in [('DMS_1', 49998000, 49999500, 5),
                              ('DDM_1', 599900000, 599960000, 100)]:
    rows = []
    labels = {}
    for name, x, y, axis, negative in configs:
        for value in range(start, stop + 1, step):
            minute = 600000000 - value if negative else value
            xm, ym = (minute, 100000000) if axis == 'x' else (100000000, minute)
            index = len(rows) + 1
            rows.append((index, coord(x, xm, y, ym)))
            labels[index] = (name, axis, value)
    src = '/tmp/unigis-carry-sweep-' + fmt + '.csv'
    with open(P(src), 'w', newline='') as f:
        w = csv.writer(f); w.writerow(['ID', 'COORD']); w.writerows(rows)
    out = 'memory/carry_sweep_' + fmt
    if arcpy.Exists(out): arcpy.management.Delete(out)
    result = arcpy.management.ConvertCoordinateNotation(P(src), out, 'COORD', None,
        'GEOREF', fmt, '', arcpy.SpatialReference(4326))
    field = 'DMSLatLon' if fmt == 'DMS_1' else 'DDMLatLon'
    with arcpy.da.SearchCursor(out, ['ID', field]) as cursor:
        values = [{'group': labels[i][0], 'axis': labels[i][1],
                   'minute_units': labels[i][2], 'text': text} for i, text in cursor]
    all_results.append({'format': fmt, 'values': values, 'messages': result.getMessages(1)})
    with open(P('/tmp/unigis-carry-sweep-progress.json'), 'w') as progress:
        json.dump(all_results, progress)
print('Saved', len(all_results), 'batches to /tmp/unigis-carry-sweep-progress.json')
