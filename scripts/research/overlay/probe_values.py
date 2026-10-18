import arcpy, json, random, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
arcpy.env.cellSize = None
D = '/tmp/claude-1000/ov'
random.seed(11)
LL = arcpy.Point(500000, 5000000)
def mk(name, f, dtype=np.float32, nodata_frac=0.1, nd=-9999.0, n=8):
    a = np.array([[f(r, c) for c in range(n)] for r in range(n)], dtype=dtype)
    mask = np.array([[random.random() < nodata_frac for c in range(n)] for r in range(n)])
    r = arcpy.NumPyArrayToRaster(a, LL, 30, 30, nd)
    if nodata_frac:
        a2 = a.astype(np.float64); a2[mask] = nd
        r = arcpy.NumPyArrayToRaster(a2.astype(dtype), LL, 30, 30, nd)
    path = P(D + '/' + name + '.tif'); r.save(path)
    return path, a, mask
rf = lambda r, c: round(random.uniform(1, 9), 2)
ri = lambda r, c: random.randint(1, 9)
pa, A, mA = mk('A', rf)
pb, B, mB = mk('B', rf)
pc, C, mC = mk('C', ri, np.int32)
pd, Dd, mD = mk('Dr', lambda r, c: random.randint(1, 4), np.int32, 0.05)
res = {}
def dump(key, ras, extra=None):
    r = arcpy.Raster(ras) if isinstance(ras, str) else ras
    arr = arcpy.RasterToNumPyArray(r, nodata_to_value=-9999)
    e = r.extent
    res[key] = {'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': r.width, 'rows': r.height, 'pixel': r.pixelType, 'values': arr.tolist()}
def go(key, fn):
    try:
        r = fn(); p = P(D + '/o_' + key + '.tif'); r.save(p); dump(key, p); print('ok', key, res[key]['pixel'])
    except Exception as e:
        print('FAIL', key, str(e)[:100])
# inputs
for k, (p, a, m) in {'A': (pa, A, mA), 'B': (pb, B, mB), 'C': (pc, C, mC), 'D': (pd, Dd, mD)}.items():
    dump('in_' + k, p)
# cell statistics
for st in ['SUM', 'MEAN', 'MAXIMUM', 'MINIMUM', 'MEDIAN', 'MAJORITY', 'MINORITY', 'RANGE', 'STD', 'VARIETY']:
    for ig in ['DATA', 'NODATA']:
        go('cs_%s_%s_f' % (st, ig), lambda st=st, ig=ig: CellStatistics([pa, pb, pc], st, ig))
        go('cs_%s_%s_i' % (st, ig), lambda st=st, ig=ig: CellStatistics([pc, pd], st, ig))
# weighted sum
go('wsum', lambda: WeightedSum(WSTable([[pa, 'VALUE', 0.5], [pb, 'VALUE', 0.3], [pc, 'VALUE', 1.2]])))
go('wsum_i', lambda: WeightedSum(WSTable([[pc, 'VALUE', 2], [pd, 'VALUE', 3]])))
# highest / lowest position
go('hp', lambda: HighestPosition([pa, pb, pc]))
go('lp', lambda: LowestPosition([pa, pb, pc]))
go('hp_i', lambda: HighestPosition([pc, pd]))
# combine
go('combine', lambda: Combine([pc, pd]))
# weighted overlay
remapC = RemapValue([[1, 1], [2, 1], [3, 3], [4, 5], [5, 5], [6, 7], [7, 9], [8, 9], [9, "NODATA"]])
remapD = RemapValue([[1, 2], [2, 4], [3, 6], [4, 8]])
go('wo', lambda: WeightedOverlay(WOTable([[pc, 60, 'VALUE', remapC], [pd, 40, 'VALUE', remapD]], [1, 9, 1])))
go('wo_ev', lambda: WeightedOverlay(WOTable([[pc, 70, 'VALUE', remapC], [pd, 30, 'VALUE', remapD]], [1, 9, 1])))
# fuzzy membership
fz = {
 'linear': FuzzyLinear(3, 7), 'linear_dec': FuzzyLinear(7, 3), 'gaussian': FuzzyGaussian(5, 0.2), 'gaussian_s': FuzzyGaussian(5, 0.6),
 'large': FuzzyLarge(5, 5), 'large_s': FuzzyLarge(4, 8), 'small': FuzzySmall(5, 5), 'small_s': FuzzySmall(6, 2),
 'mslarge': FuzzyMSLarge(1.0, 1.5), 'mssmall': FuzzyMSSmall(1.0, 1.5), 'near': FuzzyNear(5, 0.3), 'near_s': FuzzyNear(5, 0.05),
}
for k, f in fz.items():
    go('fm_' + k, lambda f=f: FuzzyMembership(pa, f))
go('fm_c_large', lambda: FuzzyMembership(pc, FuzzyLarge(5, 5)))
# fuzzy overlay
m1 = FuzzyMembership(pa, FuzzyLinear(3, 7)); m2 = FuzzyMembership(pb, FuzzyGaussian(5, 0.2)); m3 = FuzzyMembership(pc, FuzzyLarge(5, 5))
m1.save(P(D + '/m1.tif')); m2.save(P(D + '/m2.tif')); m3.save(P(D + '/m3.tif'))
ms = [P(D + '/m1.tif'), P(D + '/m2.tif'), P(D + '/m3.tif')]
for t in ['AND', 'OR', 'PRODUCT', 'SUM']:
    go('fo_' + t, lambda t=t: FuzzyOverlay(ms, t))
for g in [0.0, 0.3, 0.7, 1.0]:
    go('fo_gamma_%s' % g, lambda g=g: FuzzyOverlay(ms, 'GAMMA', g))
json.dump(res, open(P(D + '/overlay.json'), 'w'))
