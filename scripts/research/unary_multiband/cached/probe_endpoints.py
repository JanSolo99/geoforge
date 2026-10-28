import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-cached-int-multiband/'
for name in ['i8_band1','i8_band2']:
 out=base+name+'_Abs_arcgis.tif'
 arcpy.sa.Abs(P(base+name+'.tif')).save(P(out))
 raster=arcpy.Raster(P(out))
 print(json.dumps({'case':name,'op':'Abs','type':raster.pixelType,'nodata':raster.noDataValue}))
