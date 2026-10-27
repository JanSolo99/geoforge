import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
src='/tmp/unigis_cached_symmetric.tif'
arcpy.management.CalculateStatistics(P(src),1,1)
for tool in ['Abs','Negate','Square']:
 out='/tmp/unigis_cached_symmetric_'+tool+'.tif'
 result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
 r=arcpy.Raster(P(out))
 print(json.dumps({'tool':tool,'type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
 del r
