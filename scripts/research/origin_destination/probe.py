"""ArcGIS bridge: create typed projected fixtures and probe planar links twice."""
import json
arcpy.env.overwriteOutput = True
root = '/mnt/media/ArcGIS_Linux'
folder = P(root + '/testdata/geoproc')
sr = arcpy.SpatialReference(32633)
for name, rows in [
    ('od_origins', [(0, 0, 'A'), (100, 0, 'B'), (0, 100, 'A')]),
    ('od_destinations', [(30, 40, 'A'), (110, 0, 'B'), (0, 0, 'A'), (300, 300, 'C')]),
]:
    path = folder + '/' + name + '.shp'
    arcpy.management.CreateFeatureclass(folder, name + '.shp', 'POINT', spatial_reference=sr)
    arcpy.management.AddField(path, 'GRP', 'TEXT', field_length=8)
    with arcpy.da.InsertCursor(path, ['SHAPE@XY', 'GRP']) as cursor:
        for x, y, group in rows:
            cursor.insertRow(((500000+x, 5000000+y), group))
orig = folder + '/od_origins.shp'
dest = folder + '/od_destinations.shp'
results = {}
for name, args in [('default', []), ('repeat', []), ('group', ['GRP', 'GRP']),
                   ('nearest', ['', '', 'PLANAR', 1]),
                   ('search', ['', '', 'PLANAR', '', 100]),
                   ('km', ['', '', 'PLANAR', 2, 0.15, 'KILOMETERS']),
                   ('same', []), ('aggregate', ['', '', 'PLANAR', '', '', '', True])]:
    out = P('/tmp/unigis-od-' + name + '.shp')
    try:
        arcpy.analysis.GenerateOriginDestinationLinks(orig, orig if name == 'same' else dest, out, *args)
        results[name] = {
            'fields': [(f.name, f.type, f.length, f.precision, f.scale) for f in arcpy.ListFields(out)],
            'rows': list(arcpy.da.SearchCursor(out, ['*', 'SHAPE@WKT'])),
            'messages': arcpy.GetMessages(),
        }
    except Exception as error:
        results[name] = {'error': str(error), 'messages': arcpy.GetMessages()}
with open(P('/tmp/unigis-od-probe.json'), 'w') as stream:
    json.dump(results, stream, indent=2)
print('Saved /tmp/unigis-od-probe.json; ' + ', '.join(results))
