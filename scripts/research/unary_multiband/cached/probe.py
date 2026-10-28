import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-cached-int-multiband/'
for name in ['u8','i8','u16','i16','u32','i32','i16_reverse']:
 source=base+name+'.tif'
 arcpy.management.CalculateStatistics(P(source))
 for op in ['Abs','Negate','Square']:
  out=base+name+'_'+op+'.tif'
  try:
   getattr(arcpy.sa,op)(P(source)).save(P(out))
   raster=arcpy.Raster(P(out))
   print(json.dumps({'case':name,'op':op,'bands':raster.bandCount,'type':raster.pixelType,'nodata':raster.noDataValue}))
  except Exception as error:print(json.dumps({'case':name,'op':op,'error':str(error)}))
