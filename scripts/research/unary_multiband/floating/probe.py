import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-float-unary-more/'
for name in ['float32_raw','float32_cached','float64_raw','float64_cached']:
 source=base+name+'.tif'
 if name.endswith('_cached'):arcpy.management.CalculateStatistics(P(source))
 for op in ['SquareRoot','Ln','Exp']:
  out=base+name+'_'+op+'.tif'
  try:
   getattr(arcpy.sa,op)(P(source)).save(P(out))
   raster=arcpy.Raster(P(out))
   print(json.dumps({'case':name,'op':op,'bands':raster.bandCount,'type':raster.pixelType,'nodata':raster.noDataValue}))
  except Exception as error:print(json.dumps({'case':name,'op':op,'error':str(error)}))
