import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-cached-int-multiband/'
for name in ['i8_nonpositive','i16_nonpositive','u32_overflow']:
 source=base+name+'.tif'
 arcpy.management.CalculateStatistics(P(source))
 for op in ['Abs','Negate','Square']:
  out=base+name+'_'+op+'.tif'
  try:
   getattr(arcpy.sa,op)(P(source)).save(P(out))
   raster=arcpy.Raster(P(out))
   print(json.dumps({'case':name,'op':op,'bands':raster.bandCount,'type':raster.pixelType,'nodata':raster.noDataValue}))
  except Exception as error:print(json.dumps({'case':name,'op':op,'error':str(error)}))
source=base+'u32_band2.tif';out=base+'u32_band2_Square_arcgis.tif'
arcpy.sa.Square(P(source)).save(P(out))
raster=arcpy.Raster(P(out));print(json.dumps({'case':'u32_band2','op':'Square','bands':raster.bandCount,'type':raster.pixelType,'nodata':raster.noDataValue}))
