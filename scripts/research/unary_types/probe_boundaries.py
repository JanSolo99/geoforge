import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name,tool in [('square_i32','Square'),('square_u32','Square'),('negate_u32','Negate')]:
 src='/tmp/unigis_unary_boundary_'+name+'.tif'
 out='/tmp/unigis_unary_boundary_result_'+name+'.tif'
 result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
 r=arcpy.Raster(P(out))
 print(json.dumps({'input':name,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
 del r
