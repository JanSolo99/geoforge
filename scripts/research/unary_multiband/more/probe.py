import json
arcpy.env.overwriteOutput=True
for name in ['float32_raw','float32_cached','float64_raw','float64_cached']:
 source='/tmp/unigis-float-unary-more/'+name+'.tif'
 for op in ['Log2','Sin','RoundUp']:
  out='/tmp/unigis-float-unary-next/'+name+'_'+op+'.tif'
  result=getattr(arcpy.gp,op+'_sa')(P(source),P(out))
  raster=arcpy.Raster(P(out))
  print(json.dumps({'case':name,'op':op,'type':raster.pixelType,'bands':raster.bandCount,'nodata':raster.noDataValue,'messages':result.getMessages()}))
