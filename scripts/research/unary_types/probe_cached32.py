import arcpy,json,os,shutil
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name in ['u8','nd_u8','u32','nd_u32','i32','nd_i32','i32_positive_max','i32_negative_min','i32_negative_max','i32_ndmin']:
 src='/tmp/unigis_unary_cached32_source_'+name+'.tif'
 shutil.copyfile(P('/tmp/unigis_unary_cached32_base_'+name+'.tif'),P(src))
 if os.path.exists(P(src+'.aux.xml')):os.remove(P(src+'.aux.xml'))
 arcpy.management.CalculateStatistics(P(src),1,1)
 for tool in ['Abs','Negate','Square']:
  try:
   out='/tmp/unigis_unary_cached32_'+tool+'_'+name+'.tif'
   result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
   r=arcpy.Raster(P(out))
   print(json.dumps({'input':name,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
   del r
  except Exception as e:print(json.dumps({'input':name,'tool':tool,'status':'failed','error':str(e)}))
