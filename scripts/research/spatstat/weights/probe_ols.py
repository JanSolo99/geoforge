import arcpy, json, random, math
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/sw2'
sr = arcpy.SpatialReference(32632); S = arcpy.stats
random.seed(17)
fc = arcpy.management.CreateFeatureclass(P(D), 'ols.shp', 'POINT', spatial_reference=sr)
for f, t in [('UID', 'LONG'), ('Y', 'DOUBLE'), ('X1', 'DOUBLE'), ('X2', 'DOUBLE'), ('X3', 'LONG')]:
    arcpy.management.AddField(fc, f, t)
rows = []
for i in range(60):
    x, y = 500000 + random.uniform(0, 3000), 5000000 + random.uniform(0, 2000)
    x1 = round(random.uniform(0, 10), 3); x2 = round(random.gauss(50, 12), 3); x3 = random.randint(0, 5)
    yy = round(3 + 1.7 * x1 - 0.25 * x2 + 0.8 * x3 + random.gauss(0, 2 + 0.3 * x1) + (x - 500000) / 1000, 3)
    rows.append((x, y, 1000 + i * 7, yy, x1, x2, x3))
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'UID', 'Y', 'X1', 'X2', 'X3']) as c:
    for r in rows: c.insertRow([(r[0], r[1])] + list(r[2:]))
def dump(path, geom=False):
    d = arcpy.Describe(path)
    names = [f.name for f in d.fields if f.type != 'Geometry']
    return {'fields': [(f.name, f.type, f.length, f.precision, f.scale, f.aliasName) for f in d.fields], 'rows': [list(x) for x in arcpy.da.SearchCursor(path, names)]}
out = {'data': rows}
src = P(D) + '/ols.shp'
for key, ex in [('three', 'X1;X2;X3'), ('one', 'X1')]:
    o, ct, dt = P(D) + '/ols_%s.shp' % key, P(D) + '/ols_%s_coef.dbf' % key, P(D) + '/ols_%s_diag.dbf' % key
    r = S.OrdinaryLeastSquares(src, 'UID', o, 'Y', ex, ct, dt)
    out[key] = {'fc': dump(o), 'coef': dump(ct), 'diag': dump(dt), 'msg': r.getMessages(), 'outputs': [str(r.getOutput(i)) for i in range(r.outputCount)]}
    print(key, out[key]['coef']['rows'], out[key]['diag']['rows'][:30])
o = P(D) + '/ols_gdb.gdb'
if not arcpy.Exists(o): arcpy.management.CreateFileGDB(P(D), 'ols_gdb.gdb')
r = S.OrdinaryLeastSquares(src, 'UID', o + '/fc', 'Y', 'X1;X2;X3', o + '/coef', o + '/diag')
out['gdb'] = {'fc': [(f.name, f.type, f.length) for f in arcpy.Describe(o + '/fc').fields], 'coef': [(f.name, f.type, f.length) for f in arcpy.Describe(o + '/coef').fields], 'diag': [(f.name, f.type, f.length) for f in arcpy.Describe(o + '/diag').fields]}
r = S.SpatialAutocorrelation(P(D) + '/ols_three.shp', 'StdResid', 'NO_REPORT', 'INVERSE_DISTANCE', 'EUCLIDEAN_DISTANCE', 'ROW')
out['resid_moran'] = [r.getOutput(i) for i in range(3)]
print(out['three']['msg']); print(out['gdb'])
json.dump(out, open(P(D) + '/ols.json', 'w'), default=str)
