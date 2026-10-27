import arcpy,json,os,shutil
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
root='/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/'
for tool in ['Abs','Negate','Square']:
 for kind in ['u8','i8','u16','i16','u32','i32','f32','f64']:
  for prefix in ['', 'nd_']:
   name=prefix+kind
   src='/tmp/unigis_unary_source_'+name+'.tif'
   if not os.path.exists(P(src)):shutil.copyfile(P(root+name+'.tif'),P(src))
   out='/tmp/unigis_unary_'+tool+'_'+name+'.tif'
   try:
    result=getattr(arcpy.gp,tool+'_sa')(P(src),P(out))
    r=arcpy.Raster(P(out))
    print(json.dumps({'tool':tool,'input':name,'status':'succeeded','pixel_type':r.pixelType,'nodata':r.noDataValue,'values':arcpy.RasterToNumPyArray(r).tolist()}))
    del r
   except Exception as e:print(json.dumps({'tool':tool,'input':name,'status':'failed','error':str(e)}))
