import arcpy, json, math
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/idw'
sr = arcpy.SpatialReference(32632)
def mk(name, pts):
    fc = arcpy.management.CreateFeatureclass(P(D), name + '.shp', 'POINT', spatial_reference=sr)
    arcpy.management.AddField(fc, 'V', 'DOUBLE')
    with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'V']) as c:
        for x, y, v in pts: c.insertRow([(x, y), v])
    return P(D) + '/' + name + '.shp'
res = []
for (W, H) in [(200, 200), (400, 100), (100, 400), (300, 300), (600, 200)]:
    for off in (0.5, 1.0, 2.0):
        x0, y0 = 500000.0, 5000000.0
        t = off * (W + H)
        pts = [(x0 + 5, y0 + 5, 10.0), (x0 + W - 5, y0 + H - 5, 10.0), (x0 + W / 2, y0 + H + t, 90.0)]
        cnt = len(res); src = mk('s3', pts)
        arcpy.env.extent = arcpy.Extent(x0, y0, x0 + W, y0 + H)
        cell = 10
        try:
            r = Idw(src, 'V', cell)
        except Exception as ex:
            print('FAIL', W, H, off, ex, arcpy.GetMessages()); continue
        r.save(P(D + '/o3.tif'))
        ras = arcpy.Raster(P(D + '/o3.tif')); e = ras.extent
        arr = arcpy.RasterToNumPyArray(ras, nodata_to_value=-9999)
        res.append({'W': W, 'H': H, 'off': off, 'pts': pts, 'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': ras.width, 'rows': ras.height, 'values': [[float(v) for v in row] for row in arr]})
        print('ok', W, H, off)
json.dump(res, open(P(D + '/idw3.json'), 'w'))
