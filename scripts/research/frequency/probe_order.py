import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
def src(n):return P('/tmp/unigis_freq_broadcast_'+n+'.tif')
cases=[('single_then_two','single',['single','list2'],'MULTI_BAND'),('two_then_single','single',['list2','single'],'MULTI_BAND'),('two_then_three','single',['list2','three'],'MULTI_BAND'),('three_then_two','single',['three','list2'],'MULTI_BAND'),('three_value','three',['value2'],'MULTI_BAND')]
for tool in ['EqualToFrequency']:
 for name,value,rasters,multi in cases:
  try:
   out=P('/tmp/unigis_freq_broadcast_'+tool+'_'+name+'.tif')
   result=getattr(arcpy.gp,tool+'_sa')(src(value),';'.join(src(n) for n in rasters),out,multi)
   r=arcpy.Raster(out)
   print(json.dumps({'tool':tool,'case':name,'status':'succeeded','bands':r.bandCount,'pixel_type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
   del r
  except Exception as e:print(json.dumps({'tool':tool,'case':name,'status':'failed','error':str(e)}))
