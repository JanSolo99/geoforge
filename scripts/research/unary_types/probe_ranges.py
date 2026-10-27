import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name in ['i16_cross3','i16_cross1','i16_neg1to3','u32_max']:
 src='/tmp/unigis_unary_ranges_source_'+name+'.tif'
 arcpy.management.CalculateStatistics(P(src),1,1)
 for tool in ['Abs','Negate','Square']:
  out='/tmp/unigis_unary_ranges_'+tool+'_'+name+'.tif'
  result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
  r=arcpy.Raster(P(out))
  print(json.dumps({'input':name,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
  del r
