"""Check grid-to-grid output fields against ArcGIS without modifying the reference manifest."""
import json

results = []
for source in ['GARS', 'GEOREF']:
    path = '/mnt/media/ArcGIS_Linux/testdata/geoproc/grid_input_' + source.lower() + '.csv'
    for target in ['GARS', 'GEOREF']:
        output = 'memory/grid_outputs'
        if arcpy.Exists(output):
            arcpy.management.Delete(output)
        result = arcpy.management.ConvertCoordinateNotation(
            P(path), output, 'COORD', None, source, target, '', arcpy.SpatialReference(4326))
        with arcpy.da.SearchCursor(output, ['ID', target]) as cursor:
            results.append({'source': source, 'target': target, 'rows': list(cursor),
                            'messages': result.getMessages(1)})
print(json.dumps(results))
