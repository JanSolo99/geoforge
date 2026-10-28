import json
arcpy.env.overwriteOutput=True
base='/tmp/unigis-u32-export-controls/'
for name in ['u32_neg_float_edge','u32_neg_near_min','u32_positive_edge','i32_neg_float_edge','i32_neg_near_min','f64_neg_float_edge']:
 src=base+name+'.tif';out=base+name+'_result.tif'
 arcpy.management.CopyRaster(P(src),P(out))
 ras=arcpy.Raster(P(out))
 print(json.dumps({'profile':name,'type':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
