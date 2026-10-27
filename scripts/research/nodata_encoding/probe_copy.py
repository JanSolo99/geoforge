import arcpy,json,shutil
arcpy.env.overwriteOutput=True
cases=[('u8_300','nd_u8','300','8_BIT_UNSIGNED'),('u8_fraction','nd_u8','300.5','8_BIT_UNSIGNED'),('i16_fraction','nd_f32','','16_BIT_SIGNED'),('i16_big','nd_f32','3.4e38','16_BIT_SIGNED'),('i16_double','nd_f64','-1.7976931348623157e308','16_BIT_SIGNED'),('i16_inrange_fraction','nd_f32','-9999.5','16_BIT_SIGNED')]
for name,source,nd,pixel in cases:
 src='/tmp/unigis_nd_encoding_source_'+name+'.tif'
 shutil.copyfile(P('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/'+source+'.tif'),P(src))
 out='/tmp/unigis_nd_encoding_'+name+'.tif'
 try:
  result=arcpy.management.CopyRaster(P(src),P(out),nodata_value=nd,pixel_type=pixel)
  r=arcpy.Raster(P(out))
  print(json.dumps({'case':name,'status':'succeeded','type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
  del r
 except Exception as e:print(json.dumps({'case':name,'status':'failed','error':str(e)}))
