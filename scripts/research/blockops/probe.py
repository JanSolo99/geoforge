import arcpy, json, random, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
arcpy.env.cellSize = None
D = '/tmp/claude-1000/sb'
random.seed(9)
LL = arcpy.Point(500000, 5000000)
def mk(name, a, nd):
    r = arcpy.NumPyArrayToRaster(a, LL, 30, 30, nd); p = P(D + '/' + name + '.tif'); r.save(p); return p
n = 13
fl = np.array([[round(random.uniform(0, 100), 2) for c in range(n)] for r in range(10)], dtype=np.float32)
fl[2, 3] = -9999; fl[5, 6] = -9999; fl[9, 12] = -9999
it = np.array([[random.choice([1, 1, 1, 2, 2, 3, 4]) for c in range(n)] for r in range(10)], dtype=np.int32)
it[3, 4] = -9999; it[3, 5] = -9999; it[7, 0] = -9999
cls = np.zeros((10, n), dtype=np.int32); cls[:] = -9999
for r in range(10):
    for c in range(n):
        cls[r, c] = random.choice([1, 2, 3]) if random.random() < 0.55 else -9999
FP = mk('fl', fl, -9999.0); IP = mk('it', it, -9999); CP = mk('cls', cls, -9999)
res = {'fl': fl.tolist(), 'it': it.tolist(), 'cls': cls.tolist()}
def dump(key, ras):
    p = P(D + '/o.tif'); ras.save(p); r = arcpy.Raster(p); e = r.extent
    arr = arcpy.RasterToNumPyArray(r, nodata_to_value=-123456789)
    res[key] = {'pixel': r.pixelType, 'nodata': r.noDataValue, 'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': r.width, 'rows': r.height, 'values': arr.tolist()}
def go(key, fn):
    try: dump(key, fn()); print('ok', key, res[key]['pixel'], res[key]['nodata'], res[key]['cols'], res[key]['rows'])
    except Exception as e: print('FAIL', key, str(e)[:120])
for src, tag in ((FP, 'f'), (IP, 'i')):
    for st in ['MEAN', 'MAXIMUM', 'MEDIAN', 'MINIMUM', 'SUM']:
        for ig in ['DATA', 'NODATA']:
            go('agg_%s_%s_%s_%s' % (tag, st, ig, 'exp'), lambda src=src, st=st, ig=ig: Aggregate(src, 3, st, 'EXPAND', ig))
            go('agg_%s_%s_%s_%s' % (tag, st, ig, 'trunc'), lambda src=src, st=st, ig=ig: Aggregate(src, 3, st, 'TRUNCATE', ig))
    for st in ['MEAN', 'MAJORITY', 'MAXIMUM', 'MEDIAN', 'MINIMUM', 'MINORITY', 'RANGE', 'STD', 'SUM', 'VARIETY']:
        for nb in ['Rectangle 3 3 CELL', 'Circle 2 CELL']:
            go('blk_%s_%s_%s' % (tag, st, nb.split()[0]), lambda src=src, st=st, nb=nb: BlockStatistics(src, nb, st, 'DATA'))
for m in ['EQUAL_INTERVAL', 'EQUAL_AREA', 'NATURAL_BREAKS', 'STANDARD_DEVIATION', 'GEOMETRICAL_INTERVAL', 'SMOOTH_NATURAL_BREAKS']:
    go('slice_' + m, lambda m=m: Slice(FP, 5, m))
    go('slice_i_' + m, lambda m=m: Slice(IP, 3, m))
go('slice_base', lambda: Slice(FP, 4, 'EQUAL_INTERVAL', 10))
go('pick', lambda: Pick(IP, [FP, FP, IP, IP]))
go('pick_const', lambda: Pick(IP, [10, 20, 30, 40]))
for nb in ['4', '8']:
    go('expand_' + nb, lambda nb=nb: Expand(CP, 1, [1, 2]))
    go('expand_all', lambda: Expand(CP, 2, [1, 2, 3]))
    go('shrink', lambda: Shrink(CP, 1, [1, 2]))
    go('shrink_all', lambda: Shrink(CP, 2, [1, 2, 3]))
    go('rg4', lambda: RegionGroup(CP, 'FOUR', 'WITHIN'))
    go('rg8', lambda: RegionGroup(CP, 'EIGHT', 'WITHIN'))
    go('rg8_cross', lambda: RegionGroup(CP, 'EIGHT', 'CROSS'))
    go('rg4_ign', lambda: RegionGroup(CP, 'FOUR', 'WITHIN', 'NO_LINK'))
    go('maj4', lambda: MajorityFilter(IP, 'FOUR', 'MAJORITY'))
    go('maj8', lambda: MajorityFilter(IP, 'EIGHT', 'MAJORITY'))
    go('maj4_half', lambda: MajorityFilter(IP, 'FOUR', 'HALF'))
    go('maj8_half', lambda: MajorityFilter(IP, 'EIGHT', 'HALF'))
    break
json.dump(res, open(P(D) + '/sb1.json', 'w'))
