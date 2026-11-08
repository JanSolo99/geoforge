import arcpy,json,os,shutil
root=P('/mnt/media/ArcGIS_Linux/scripts/research/set_raster_properties')
for label,src,kwargs in [
 ('i16_nd','unary_cached_multiband/i16.tif',dict(statistics='1 -10 20 2 -3')),
 ('f32_nd','dem.tif',dict(nodata='1 2.123456789123456')),
 ('negative_nd','classes_u8.tif',dict(nodata='1 -2.5')),
 ('half_nd','classes_u8.tif',dict(nodata='1 3.5')),
 ('stats_duplicate','classes_u8.tif',dict(statistics='1 1 100 20 3;1 2 200 30 4')),
 ('stats_two','rgb_pixel_u8.tif',dict(statistics='1 1 50 20 3;3 5 250 30 4',data_type='VECTOR_UV')),
 ('empty_nd','classes_u8.tif',dict(nodata='1 #')),
]:
 folder=root+'/scratch/'+label
 os.makedirs(folder,exist_ok=True)
 path=folder+'/input.tif';source=P('/mnt/media/ArcGIS_Linux/testdata/raster/'+src)
 shutil.copyfile(source,path)
 if os.path.exists(source+'.aux.xml'):shutil.copyfile(source+'.aux.xml',path+'.aux.xml')
 try:
  r=arcpy.management.SetRasterProperties(path,**kwargs)
  result=dict(case=label,source=src,kwargs=kwargs,status='succeeded',messages=r.getMessages())
 except Exception as e:result=dict(case=label,source=src,kwargs=kwargs,status='failed',messages=str(e))
 if os.path.exists(path+'.aux.xml'):result['aux']=open(path+'.aux.xml').read()
 print(json.dumps(result))
