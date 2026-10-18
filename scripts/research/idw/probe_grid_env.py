import arcpy, json, random
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/idw'
res = {'grid': [], 'env': {}}
sr = arcpy.SpatialReference(32632)
def mk(name, pts):
    fc = arcpy.management.CreateFeatureclass(P(D), name + '.shp', 'POINT', spatial_reference=sr)
    arcpy.management.AddField(fc, 'V', 'DOUBLE')
    with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'V']) as c:
        for x, y, v in pts: c.insertRow([(x, y), v])
    return P(D) + '/' + name + '.shp'
def info(path):
    r = arcpy.Raster(path); e = r.extent
    return {'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': r.width, 'rows': r.height, 'cell': r.meanCellWidth}
for seed in range(1, 9):
    random.seed(seed)
    w, h = random.uniform(100, 900), random.uniform(100, 900)
    ox, oy = 500000 + random.uniform(-50, 50), 5000000 + random.uniform(-50, 50)
    pts = [(ox + random.uniform(0, w), oy + random.uniform(0, h), random.uniform(0, 100)) for _ in range(10)]
    src = mk('g%d' % seed, pts)
    xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
    for cell in (7, 13, 20, 33.3):
        arcpy.env.cellSize = None
        r = Idw(src, 'V', cell)
        r.save(P(D + '/o.tif'))
        res['grid'].append({'seed': seed, 'cell': cell, 'pts': [min(xs), min(ys), max(xs), max(ys)], **info(P(D + '/o.tif'))})
# default cell size
random.seed(3)
pts = [(500000 + random.uniform(0, 800), 5000000 + random.uniform(0, 300), random.uniform(0, 100)) for _ in range(10)]
src = mk('dflt', pts)
xs = [p[0] for p in pts]; ys = [p[1] for p in pts]
r = Idw(src, 'V'); r.save(P(D + '/o.tif'))
res['default_cell'] = {'pts': [min(xs), min(ys), max(xs), max(ys)], **info(P(D + '/o.tif'))}
# environments
for name, setter in {
  'extent': lambda: setattr(arcpy.env, 'extent', arcpy.Extent(500103.3, 5000041.7, 500377.9, 5000290.2)),
  'cellsize_env': lambda: setattr(arcpy.env, 'cellSize', 15),
  'snap': lambda: (setattr(arcpy.env, 'snapRaster', P(D + '/snap.tif'))),
}.items():
    pass
snap = arcpy.sa.CreateConstantRaster(1, 'INTEGER', 20, arcpy.Extent(499993, 4999987, 500453, 5000367)); snap.save(P(D + '/snap.tif'))
mask = arcpy.management.CreateFeatureclass(P(D), 'mask.shp', 'POLYGON', spatial_reference=sr)
with arcpy.da.InsertCursor(mask, ['SHAPE@']) as c:
    c.insertRow([arcpy.Polygon(arcpy.Array([arcpy.Point(500150, 5000080), arcpy.Point(500420, 5000100), arcpy.Point(500300, 5000300), arcpy.Point(500160, 5000250)]), sr)])
random.seed(5)
pts = [(500000 + random.uniform(0, 600), 5000000 + random.uniform(0, 400), random.uniform(0, 100)) for _ in range(12)]
src = mk('envp', pts)
envcases = {
  'extent': {'extent': arcpy.Extent(500103.3, 5000041.7, 500377.9, 5000290.2)},
  'snap': {'snapRaster': P(D + '/snap.tif')},
  'mask': {'mask': P(D) + '/mask.shp'},
  'cell_env': {'cellSize': 15},
}
for name, env in envcases.items():
    arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None; arcpy.env.cellSize = None
    for k, v in env.items(): setattr(arcpy.env, k, v)
    r = Idw(src, 'V', 20 if name != 'cell_env' else None)
    r.save(P(D + '/e_%s.tif' % name))
    ras = arcpy.Raster(P(D + '/e_%s.tif' % name))
    arr = arcpy.RasterToNumPyArray(ras, nodata_to_value=-9999)
    res['env'][name] = {**info(P(D + '/e_%s.tif' % name)), 'values': [[float(v) for v in row] for row in arr]}
    print(name, res['env'][name]['extent'], res['env'][name]['cols'], res['env'][name]['rows'])
res['env_points'] = pts
json.dump(res, open(P(D + '/idw2.json'), 'w'))
