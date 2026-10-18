import arcpy, json, random, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
arcpy.env.cellSize = None
D = '/tmp/claude-1000/sa'
random.seed(2)
LL = arcpy.Point(500000, 5000000)
def mk(name, vals, dtype, nd):
    a = np.array(vals, dtype=dtype)
    r = arcpy.NumPyArrayToRaster(a, LL, 30, 30, nd); p = P(D + '/' + name + '.tif'); r.save(p); return p
fvals = [[-2.5, -1.0, -0.5, 0.0, 0.25, 0.5, 1.0, 2.0], [3.7, 10.0, -9.99, 100.0, 1e-3, -1e-3, 0.99, 1.5], [-3.0, 4.0, 2.0, 8.0, -8.0, 0.75, -0.75, 6.25], [1e6, -1e6, 5.5, 4.5, -4.5, -5.5, 0.1, -0.1], [-9999.0, 1.0, -9999.0, 2.0, 3.0, -9999.0, 0.2, 7.0], [0.2, 0.4, 0.6, 0.8, 1.0, -0.2, -0.4, -1.0]]
ivals = [[-5, -1, 0, 1, 2, 3, 7, 100], [0, 0, 1, 1, -1, -1, 5, -5], [255, 256, 1000, -1000, 3, 4, 5, 6], [10, 20, 30, 40, 50, 60, 70, 80], [-9999, 2, -9999, 3, 4, -9999, 5, 6], [8, 7, 6, 5, 4, 3, 2, 1]]
bvals = [[1, 2, 3, 4, 5, 6, 7, 8], [0, 1, 2, 3, 4, 5, 6, 7], [2, 2, 2, 2, 2, 2, 2, 2], [-3, -2, -1, 0, 1, 2, 3, 4], [1, -9999, 3, 4, -9999, 6, 7, 8], [3, 3, 3, 3, 3, 3, 3, 3]]
F = mk('F', fvals, np.float32, -9999.0)
Ii = mk('I', ivals, np.int32, -9999)
B = mk('B', bvals, np.int32, -9999)
G = mk('G', [[v + 0.5 if v != -9999 else v for v in row] for row in fvals], np.float32, -9999.0)
res = {'inputs': {'F': fvals, 'I': ivals, 'B': bvals}}
def dump(key, ras):
    p = P(D + '/o.tif'); ras.save(p); r = arcpy.Raster(p)
    arr = arcpy.RasterToNumPyArray(r, nodata_to_value=-123456789)
    res[key] = {'pixel': r.pixelType, 'nodata': r.noDataValue, 'values': arr.tolist()}
def go(key, fn):
    try: dump(key, fn()); print('ok', key, res[key]['pixel'], res[key]['nodata'])
    except Exception as e: print('FAIL', key, str(e)[:100])
for tag, src in (('F', F), ('I', Ii), ('B', B)):
    go('SquareRoot_' + tag, lambda src=src: SquareRoot(src)); go('Negate_' + tag, lambda src=src: Negate(src))
    go('Abs_' + tag, lambda src=src: Abs(src)); go('Square_' + tag, lambda src=src: Square(src)); go('Int_' + tag, lambda src=src: Int(src))
    go('BooleanNot_' + tag, lambda src=src: BooleanNot(src))
go('ATan2_FG', lambda: ATan2(F, G)); go('ATan2_IB', lambda: ATan2(Ii, B))
go('Power_BB', lambda: Power(B, B)); go('Times_BB', lambda: Times(B, B)); go('Mod_BB', lambda: Mod(B, B)); go('Mod_BI', lambda: Mod(B, Ii))
go('Diff_BB', lambda: Diff(B, B)); go('Over_BB', lambda: Over(B, B)); go('Over_BI', lambda: Over(B, Ii)); go('Diff_BI', lambda: Diff(B, Ii))
json.dump(res, open(P(D) + '/sa2.json', 'w'))
