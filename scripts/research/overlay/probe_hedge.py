import arcpy, json
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/ov'
pa = P(D + '/A.tif')
res = {}
for h in ['NONE', 'SOMEWHAT', 'VERY']:
    for k, f in {'linear': FuzzyLinear(3, 7), 'large': FuzzyLarge(5, 5)}.items():
        r = FuzzyMembership(pa, f, h); p = P(D + '/h_%s_%s.tif' % (k, h)); r.save(p)
        res['%s_%s' % (k, h)] = arcpy.RasterToNumPyArray(arcpy.Raster(p), nodata_to_value=-9999).tolist()
json.dump(res, open(P(D + '/hedge.json'), 'w'))
print('done')
