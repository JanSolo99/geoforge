import arcpy,json,shutil,os
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name in ['i8_narrow','i16_narrow','u16_narrow','i8_wide','i16_wide','i16_ndmin']:
 for cached in [False,True]:
  for tool in ['Negate','Square']:
   src='/tmp/unigis_unary_stats_source_'+name+'_'+str(cached)+'_'+tool+'.tif'
   shutil.copyfile(P('/tmp/unigis_unary_stats_base_'+name+'.tif'),P(src))
   if os.path.exists(P(src+'.aux.xml')):os.remove(P(src+'.aux.xml'))
   if cached:arcpy.management.CalculateStatistics(P(src),1,1)
   out='/tmp/unigis_unary_stats_'+name+'_'+str(cached)+'_'+tool+'.tif'
   try:
    result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
    r=arcpy.Raster(P(out))
    print(json.dumps({'input':name,'cached':cached,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
    del r
   except Exception as e:print(json.dumps({'input':name,'cached':cached,'tool':tool,'status':'failed','error':str(e)}))
