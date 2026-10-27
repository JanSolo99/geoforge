import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name in ['i16_tiny','i16_bit','i16_negative','f64_cached','f64_nd_cached','f64_extreme']:
 src='/tmp/unigis_unary_extra_'+name+'.tif'
 if name!='f64_extreme':arcpy.management.CalculateStatistics(P(src),1,1)
 for tool in ['Abs','Negate','Square']:
  try:
   out='/tmp/unigis_unary_extra_'+name+'_'+tool+'.tif'
   result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
   r=arcpy.Raster(P(out))
   print(json.dumps({'input':name,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
   del r
  except Exception as e:print(json.dumps({'input':name,'tool':tool,'status':'failed','error':str(e)}))
