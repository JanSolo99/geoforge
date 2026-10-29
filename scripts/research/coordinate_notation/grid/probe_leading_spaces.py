"""Read the literal DBF character value with ArcGIS, on oracle and native files."""
import json
for path in ['/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/ccn_grid_input_gars_dd_numeric.shp',
             '/tmp/unigis-grid-leading/native.shp']:
    with arcpy.da.SearchCursor(P(path), ['ID', 'COORD']) as rows:
        print(json.dumps({'path': path, 'row55': [row for row in rows if row[0] == 55]}))
