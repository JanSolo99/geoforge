import arcpy,json,shutil
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
for name in ['u16','nd_u16']:
 src='/tmp/unigis_unary_cached16_'+name+'.tif'
 shutil.copyfile(P('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/'+name+'.tif'),P(src))
 arcpy.management.CalculateStatistics(P(src),1,1)
 for tool in ['Abs','Negate','Square']:
  out='/tmp/unigis_unary_cached16_'+tool+'_'+name+'.tif'
  result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
  r=arcpy.Raster(P(out))
  print(json.dumps({'input':name,'tool':tool,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
  del r
