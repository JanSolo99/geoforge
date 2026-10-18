import arcpy, json, random
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/idw'
random.seed(7)
pts = [(500000 + random.uniform(0, 600), 5000000 + random.uniform(0, 400), round(random.uniform(10, 90), 3)) for _ in range(16)]
sr = arcpy.SpatialReference(32632)
fc = arcpy.management.CreateFeatureclass(P(D), 'pts.shp', 'POINT', spatial_reference=sr)
arcpy.management.AddField(fc, 'V', 'DOUBLE')
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'V']) as c:
    for x, y, v in pts:
        c.insertRow([(x, y), v])
cases = {
  'default': dict(cell_size=20),
  'power1': dict(cell_size=20, power=1),
  'power3': dict(cell_size=20, power=3),
  'var5': dict(cell_size=20, search_radius=RadiusVariable(5)),
  'var5_max': dict(cell_size=20, search_radius=RadiusVariable(5, 150)),
  'fixed': dict(cell_size=20, search_radius=RadiusFixed(150, 1)),
  'fixed_min3': dict(cell_size=20, search_radius=RadiusFixed(150, 3)),
  'fixed_def': dict(cell_size=20, search_radius=RadiusFixed()),
}
out = {'points': pts, 'cases': {}}
for name, kw in cases.items():
    try:
        r = Idw(P(D) + '/pts.shp', 'V', **kw)
        r.save(P(D + '/' + name + '.tif'))
        ras = arcpy.Raster(P(D + '/' + name + '.tif'))
        e = ras.extent
        arr = arcpy.RasterToNumPyArray(ras, nodata_to_value=-9999)
        out['cases'][name] = {'extent': [e.XMin, e.YMin, e.XMax, e.YMax], 'cols': ras.width, 'rows': ras.height, 'cell': ras.meanCellWidth, 'values': [[float(v) for v in row] for row in arr]}
        print('ok', name, ras.width, ras.height, e.XMin, e.YMin)
    except Exception as ex:
        print('FAIL', name, ex)
json.dump(out, open(P(D + '/idw.json'), 'w'))
