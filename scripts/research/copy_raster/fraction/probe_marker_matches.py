import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
source='/mnt/media/ArcGIS_Linux/testdata/raster/copy_fraction/pixel_ties.tif'
for marker in [2.5,-1.5,-2.5]:
 out='/tmp/unigis-copy-fraction-controls/match_'+str(marker)+'.tif'
 arcpy.management.CopyRaster(P(source),P(out),pixel_type='16_BIT_SIGNED',nodata_value=str(marker))
 r=arcpy.Raster(P(out))
 print(json.dumps({'marker':marker,'nd':r.noDataValue,'cells':arcpy.RasterToNumPyArray(r).tolist()}))
