import arcpy, json, numpy as np
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial")
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/ex'
LL = arcpy.Point(500000, 5000000)
cfgs = json.load(open(P(D + '/cfg.json')))
res = {}
for name, (H, W, cells) in cfgs.items():
    src = np.full((H, W), -9999, dtype=np.int32)
    for i, (r, c) in enumerate(cells): src[r, c] = i + 1
    sp = P(D + '/s_%s.tif' % name)
    arcpy.NumPyArrayToRaster(src, LL, 10, 10, -9999).save(sp)
    o = EucDirection(sp); op = P(D + '/d_%s.tif' % name); o.save(op)
    a = EucAllocation(sp); ap = P(D + '/a_%s.tif' % name); a.save(ap)
    res[name] = {'dir': arcpy.RasterToNumPyArray(arcpy.Raster(op), nodata_to_value=-1).tolist(),
                 'alloc': arcpy.RasterToNumPyArray(arcpy.Raster(ap), nodata_to_value=-1).tolist()}
json.dump(res, open(P(D + '/out.json'), 'w'))
print('done', len(res))
