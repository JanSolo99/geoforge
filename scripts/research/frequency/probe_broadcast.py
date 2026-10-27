import arcpy,json
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
def src(n):return P('/tmp/unigis_freq_broadcast_'+n+'.tif')
cases=[('val_broadcast','single',['list2'],'MULTI_BAND'),('list_broadcast','value2',['single'],'MULTI_BAND'),('multi_pair','value2',['list2'],'MULTI_BAND'),('single_flat','value2',['list2'],'SINGLE_BAND'),('two_lists','value2',['list2','single'],'MULTI_BAND'),('unequal','value2',['three'],'MULTI_BAND')]
for tool in ['EqualToFrequency','GreaterThanFrequency','LessThanFrequency']:
 for name,value,rasters,multi in cases:
  try:
   out=P('/tmp/unigis_freq_broadcast_'+tool+'_'+name+'.tif')
   result=getattr(arcpy.gp,tool+'_sa')(src(value),';'.join(src(n) for n in rasters),out,multi)
   r=arcpy.Raster(out)
   print(json.dumps({'tool':tool,'case':name,'status':'succeeded','bands':r.bandCount,'pixel_type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
   del r
  except Exception as e:print(json.dumps({'tool':tool,'case':name,'status':'failed','error':str(e)}))
