import json
arcpy.env.overwriteOutput=True
for marker in [-1.5,-1.75]:
 src='/mnt/media/ArcGIS_Linux/testdata/raster/copy_fraction/source_'+str(marker)+'.tif'
 for explicit in [False,True]:
  out='/tmp/unigis-copy-fraction-controls/default_u8_'+str(marker)+'_'+str(explicit)+'.tif'
  kwargs={'pixel_type':'8_BIT_UNSIGNED'}
  if explicit:kwargs['nodata_value']=str(marker)
  arcpy.management.CopyRaster(P(src),P(out),**kwargs)
  print(json.dumps({'output':out,'nd':arcpy.Raster(P(out)).noDataValue}))
