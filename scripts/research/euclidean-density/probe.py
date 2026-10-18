import arcpy, json, random, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
arcpy.env.cellSize = None
D = '/tmp/claude-1000/sc'
LL = arcpy.Point(500000, 5000000)
sr = arcpy.SpatialReference(32632)
src = np.full((12, 15), -9999, dtype=np.int32)
src[2, 3] = 1; src[7, 10] = 2; src[10, 2] = 3; src[4, 13] = 1
r = arcpy.NumPyArrayToRaster(src, LL, 30, 30, -9999); SP = P(D + '/src.tif'); r.save(SP)
random.seed(4)
pts = [(500000 + random.uniform(20, 430), 5000000 + random.uniform(20, 340), round(random.uniform(1, 9), 2)) for _ in range(12)]
fc = arcpy.management.CreateFeatureclass(P(D), 'kp.shp', 'POINT', spatial_reference=sr)
arcpy.management.AddField(fc, 'POP', 'DOUBLE')
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'POP']) as c:
    for x, y, v in pts: c.insertRow([(x, y), v])
KP = P(D) + '/kp.shp'
lf = arcpy.management.CreateFeatureclass(P(D), 'kl.shp', 'POLYLINE', spatial_reference=sr)
arcpy.management.AddField(lf, 'W', 'DOUBLE')
lines = [[(30, 40), (200, 90), (330, 60)], [(60, 300), (240, 250)], [(400, 30), (420, 320)]]
with arcpy.da.InsertCursor(lf, ['SHAPE@', 'W']) as c:
    for i, l in enumerate(lines): c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Point(500000 + x, 5000000 + y) for x, y in l]), sr), i + 1.0])
LP = P(D) + '/kl.shp'
res = {'src': src.tolist(), 'pts': pts, 'lines': lines}
def dump(key, ras):
    p = P(D + '/o.tif'); ras.save(p); r = arcpy.Raster(p); e = r.extent
    arr = arcpy.RasterToNumPyArray(r, nodata_to_value=-123456789)
    res[key] = {'pixel': r.pixelType, 'nodata': r.noDataValue, 'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': r.width, 'rows': r.height, 'values': arr.tolist()}
def go(key, fn):
    try: dump(key, fn()); print('ok', key, res[key]['pixel'], res[key]['nodata'], res[key]['cols'], res[key]['rows'])
    except Exception as e: print('FAIL', key, str(e)[:120])
go('edir', lambda: EucDirection(SP)); go('edir_md', lambda: EucDirection(SP, 200)); go('edir_cs', lambda: EucDirection(SP, None, 45))
go('eall', lambda: EucAllocation(SP)); go('eall_md', lambda: EucAllocation(SP, 150)); go('eall_cs', lambda: EucAllocation(SP, None, 45))
go('edist', lambda: EucDistance(SP)); go('edist_md', lambda: EucDistance(SP, 150))
go('kd', lambda: KernelDensity(KP, 'NONE', 30)); go('kd_pop', lambda: KernelDensity(KP, 'POP', 30))
go('kd_r', lambda: KernelDensity(KP, 'POP', 30, 120)); go('kd_dens', lambda: KernelDensity(KP, 'NONE', 30, 120, 'SQUARE_KILOMETERS'))
go('kd_dens2', lambda: KernelDensity(KP, 'POP', 30, 120, 'SQUARE_METERS')); go('kd_planar', lambda: KernelDensity(KP, 'NONE', 30, 120, 'SQUARE_KILOMETERS', 'DENSITIES', 'PLANAR'))
go('kd_exp', lambda: KernelDensity(KP, 'NONE', 30, 120, 'SQUARE_KILOMETERS', 'EXPECTED_COUNTS'))
go('kdl', lambda: KernelDensity(LP, 'NONE', 30)); go('kdl_w', lambda: KernelDensity(LP, 'W', 30, 100))
go('pd_circle', lambda: PointDensity(KP, 'POP', 30, NbrCircle(90, 'MAP'), 'SQUARE_KILOMETERS')); go('pd_rect', lambda: PointDensity(KP, 'NONE', 30, NbrRectangle(90, 90, 'MAP'), 'SQUARE_METERS'))
go('pd_ann', lambda: PointDensity(KP, 'NONE', 30, NbrAnnulus(30, 90, 'MAP'), 'SQUARE_METERS')); go('pd_wedge', lambda: PointDensity(KP, 'NONE', 30, NbrWedge(90, 0, 90, 'MAP'), 'SQUARE_METERS'))
go('ld', lambda: LineDensity(LP, 'NONE', 30, 100, 'SQUARE_KILOMETERS')); go('ld_w', lambda: LineDensity(LP, 'W', 30, 100, 'SQUARE_METERS'))
json.dump(res, open(P(D) + '/sc1.json', 'w'))
