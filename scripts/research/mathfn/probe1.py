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
un = ['Abs','Exp','Exp2','Exp10','Ln','Log2','Log10','Sqrt','Square','Sin','Cos','Tan','ASin','ACos','ATan','SinH','CosH','TanH','ASinH','ACosH','ATanH','Ceil','Floor','Neg','RoundDown','RoundUp','Int','Float']
for fn in un:
    for tag, src in (('F', F), ('I', Ii)):
        go('%s_%s' % (fn, tag), lambda fn=fn, src=src: getattr(arcpy.sa, fn)(src))
for tag, a, b in (('FG', F, G), ('FB', F, B), ('IB', Ii, B), ('II', Ii, Ii), ('BF', B, F)):
    for fn in ['Power', 'Mod', 'Divide', 'Plus', 'Minus', 'Times', 'Con']:
        if fn == 'Con': continue
        go('%s_%s' % (fn, tag), lambda fn=fn, a=a, b=b: getattr(arcpy.sa, fn)(a, b))
for tag, a, b in (('FG', F, G), ('FB', F, B), ('IB', Ii, B), ('II', Ii, Ii)):
    for fn in ['EqualTo', 'NotEqual', 'GreaterThan', 'GreaterThanEqual', 'LessThan', 'LessThanEqual', 'BooleanAnd', 'BooleanOr', 'BooleanXOr', 'BitwiseAnd', 'BitwiseOr', 'BitwiseXOr', 'Diff', 'Over', 'Test'] :
        if fn == 'Test': continue
        if fn.startswith('Bitwise') and tag.startswith('F'): continue
        go('%s_%s' % (fn, tag), lambda fn=fn, a=a, b=b: getattr(arcpy.sa, fn)(a, b))
for fn in ['BooleanNot', 'BitwiseNot']:
    for tag, src in (('F', F), ('I', Ii), ('B', B)):
        if fn == 'BitwiseNot' and tag == 'F': continue
        go('%s_%s' % (fn, tag), lambda fn=fn, src=src: getattr(arcpy.sa, fn)(src))
for fn in ['BitwiseLeftShift', 'BitwiseRightShift']:
    go('%s_B' % fn, lambda fn=fn: getattr(arcpy.sa, fn)(B, 2))
    go('%s_BB' % fn, lambda fn=fn: getattr(arcpy.sa, fn)(Ii, B))
# with constants
go('Plus_F_const', lambda: Plus(F, 1.5)); go('Power_F_const', lambda: Power(F, 2)); go('Power_I_const2', lambda: Power(Ii, 2)); go('Power_const_I', lambda: Power(2, B))
go('Mod_I_const', lambda: Mod(Ii, 3)); go('Mod_F_const', lambda: Mod(F, 0.7)); go('Mod_I_zero', lambda: Mod(Ii, 0))
go('GreaterThan_F_const', lambda: GreaterThan(F, 0)); go('EqualTo_I_const', lambda: EqualTo(Ii, 3))
go('Divide_F_zero', lambda: Divide(F, 0)); go('Divide_I_zero', lambda: Divide(Ii, 0))
json.dump(res, open(P(D) + '/sa1.json', 'w'))
