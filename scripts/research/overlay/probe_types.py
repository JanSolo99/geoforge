import arcpy, json, random, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
arcpy.env.cellSize = None
D = '/tmp/claude-1000/ov'
random.seed(5)
LL = arcpy.Point(500000, 5000000)
def mk(name, lo, hi, n=6):
    a = np.array([[random.randint(lo, hi) for c in range(n)] for r in range(n)], dtype=np.int32)
    a[0, 0] = lo; a[0, 1] = hi
    r = arcpy.NumPyArrayToRaster(a, LL, 30, 30, -2147483647)
    p = P(D + '/t_' + name + '.tif'); r.save(p)
    ras = arcpy.Raster(p)
    return p, ras.pixelType, a
ins = {}
for name, lo, hi in [('u1', 0, 1), ('u2', 0, 3), ('u4', 0, 12), ('u8', 0, 200), ('u16', 0, 1000), ('s8', -5, 5), ('s16', -1000, 1000), ('big', 0, 100000), ('neg', -100000, 50)]:
    ins[name] = mk(name, lo, hi)
    print('in', name, ins[name][1])
out = {'types': {}, 'inputs': {k: v[2].tolist() for k, v in ins.items()}}
def ty(key, fn):
    try:
        r = fn(); p = P(D + '/t_o.tif'); r.save(p); ras = arcpy.Raster(p)
        e = ras.extent
        arr = arcpy.RasterToNumPyArray(ras, nodata_to_value=-99999999)
        out['types'][key] = {'pixel': ras.pixelType, 'nodata': ras.noDataValue, 'min': float(arr.min()), 'max': float(arr.max()), 'values': arr.tolist()}
        print(key, ras.pixelType)
    except Exception as ex:
        print('FAIL', key, str(ex)[:80])
pairs = [('u1', 'u1'), ('u2', 'u8'), ('u4', 'u4'), ('u8', 'u8'), ('u8', 'u16'), ('s8', 's8'), ('s8', 'u8'), ('s16', 's16'), ('u16', 's8'), ('big', 'u1'), ('neg', 'u8'), ('s8', 'u1')]
for a, b in pairs:
    for st in ['SUM', 'MAXIMUM', 'MINIMUM', 'RANGE', 'MEDIAN', 'MAJORITY', 'VARIETY']:
        ty('cs_%s_%s_%s' % (st, a, b), lambda st=st, a=a, b=b: CellStatistics([ins[a][0], ins[b][0]], st, 'DATA'))
for a, b in pairs[:6]:
    ty('hp_%s_%s' % (a, b), lambda a=a, b=b: HighestPosition([ins[a][0], ins[b][0]]))
    ty('wsum_%s_%s' % (a, b), lambda a=a, b=b: WeightedSum(WSTable([[ins[a][0], 'VALUE', 1], [ins[b][0], 'VALUE', 1]])))
    ty('comb_%s_%s' % (a, b), lambda a=a, b=b: Combine([ins[a][0], ins[b][0]]))
# three rasters
for st in ['SUM', 'MAXIMUM', 'MEDIAN']:
    ty('cs3_%s' % st, lambda st=st: CellStatistics([ins['u8'][0], ins['u8'][0], ins['u8'][0]], st, 'DATA'))
json.dump(out, open(P(D + '/types.json'), 'w'))
