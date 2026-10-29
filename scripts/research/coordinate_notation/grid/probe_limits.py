"""Black-box ArcGIS GARS/GEOREF input grammar controls; run via arcgis-bridge/run_code.py."""
import csv
import json

controls = {
 'GARS': ['391MH31junk', '391MH3199', '391MH3junk', '391MHjunk', '391MH319', '391MH31.5', '391MH31N', '391MH-31', '3 91M H 3 1', '391MH\t31'],
 'GEOREF': ['PKAA074074073400', 'PKAA07407407340000', 'PKAA0740740734000000', 'PKAA074074073400000000', 'PKAA07407407340000000000', 'PKAA0740740734000000000000', 'PKAA074074073400000000000000', 'PKAA074074073400junk', 'PKAA0740740734.0', 'ZMQQ9999999999', 'ZMQQ6060', 'AAAA9999', 'PKAA000000', 'PKAA00000000', 'PKAA0000000000', 'PKAA000000000000', 'PKAA0 0', 'PKAA00 00'],
}
for fmt, values in controls.items():
    src = '/tmp/unigis-grid-input-' + fmt.lower() + '.csv'
    with open(P(src), 'w', newline='') as f:
        w = csv.writer(f)
        w.writerow(['ID', 'COORD'])
        w.writerows(enumerate(values, 1))
    out = 'memory/grid_' + fmt
    if arcpy.Exists(out):
        arcpy.management.Delete(out)
    result = arcpy.management.ConvertCoordinateNotation(
        P(src), out, 'COORD', None, fmt, 'DD_NUMERIC', '', arcpy.SpatialReference(4326))
    print(fmt, result.getMessages(1))
    with arcpy.da.SearchCursor(out, ['COORD', 'DDLon', 'DDLat', 'SHAPE@XY']) as cursor:
        print(json.dumps(list(cursor)))
