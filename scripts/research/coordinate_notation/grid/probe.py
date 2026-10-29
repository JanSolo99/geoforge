"""Black-box ArcGIS GARS/GEOREF input grammar controls; run via arcgis-bridge/run_code.py."""
import csv
import json

controls = {
    'GARS': ['391MH', '391MH1', '391MH2', '391MH3', '391MH4',
             '391MH11', '391MH19', '391MH21', '391MH29', '391MH31', '391MH39',
             '391MH41', '391MH49', '391mh32', '391 MH 32', ' 391MH32 ',
             '001AA11', '720QZ49', '000AA11', '721AA11', '001QA11',
             '001RA11', '001AI11', '001AO11', '391MH00', '391MH30',
             '391MH50', '391MH310', '391MH0', '391MH5', 'junk', ''],
    'GEOREF': ['PKAA0740740734', 'pkaa0740740734', 'PKAA 07407 40734',
               'PKAA0740', 'PKAA074407', 'PKAA07404073', 'PKAA074074073400',
               'PKAA07', 'PKAA', 'PK', 'PKAA07407', 'PKAA074074073',
               'PKAA6040', 'PKAA0060', 'PKAA9999', 'PKAA0000',
               'ZGQQ5999959999', 'AAAA0000', 'ZZAA0000', 'NMRA0000',
               'NMAA0000', 'NKII0000', 'PKAA-07404073', 'junk', ''],
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
