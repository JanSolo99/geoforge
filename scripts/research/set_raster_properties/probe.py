import arcpy, hashlib, json, os, shutil
root=P('/mnt/media/ArcGIS_Linux/scripts/research/set_raster_properties')
work=root+'/scratch'
if os.path.exists(work): shutil.rmtree(work)
os.makedirs(work)
arcpy.env.overwriteOutput=True
cases=[
 ('default','classes_u8.tif',{}),
 ('elevation','classes_u8.tif',dict(data_type='ELEVATION')),
 ('thematic','classes_u8.tif',dict(data_type='THEMATIC')),
 ('processed','classes_u8.tif',dict(data_type='PROCESSED')),
 ('scientific','classes_u8.tif',dict(data_type='SCIENTIFIC')),
 ('stats','classes_u8.tif',dict(statistics='1 1 250 20 3')),
 ('stats_cached','dem.tif',dict(statistics='1 50 400 5 28')),
 ('nodata','classes_u8.tif',dict(nodata='1 2')),
 ('stats_nodata','classes_u8.tif',dict(statistics='1 1 250 20 3',nodata='1 2',data_type='THEMATIC')),
 ('nd_multi','rgb_pixel_u8.tif',dict(nodata='1 3;2 4;3 5')),
 ('stats_multi','rgb_pixel_u8.tif',dict(statistics='2 5 40 12 2')),
 ('generic','classes_u8.tif',dict(data_type='GENERIC')),
 ('uv','rgb_pixel_u8.tif',dict(data_type='VECTOR_UV')),
 ('magdir','rgb_pixel_u8.tif',dict(data_type='VECTOR_MAGDIR')),
 ('date','classes_u8.tif',dict(data_type='DATE')),
 ('sar','classes_u8.tif',dict(data_type='SAR')),
 ('nd_range','classes_u8.tif',dict(nodata='1 999')),
 ('nd_fraction','classes_u8.tif',dict(nodata='1 2.5')),
 ('stats_inverted','classes_u8.tif',dict(statistics='1 250 1 20 3')),
 ('stats_fraction','classes_u8.tif',dict(statistics='1 1.25 250.5 20.123456789123456 3.23456789123456')),
 ('nd_multiple','classes_u8.tif',dict(nodata='1 \"1 2\"')), 
 ('bad_band','classes_u8.tif',dict(statistics='2 1 250 20 3')),
 ('bad_stats','classes_u8.tif',dict(statistics='1 250 1 20 -3')),
 ('bad_nd_band','classes_u8.tif',dict(nodata='2 3')),
 ('bad_type','classes_u8.tif',dict(data_type='FOO')),
]
print(json.dumps(dict(install={k:v for k,v in arcpy.GetInstallInfo().items() if k in ['Version','BuildNumber','LicenseLevel']})))
for label,source,kwargs in cases:
 folder=work+'/'+label;os.makedirs(folder)
 path=folder+'/input.tif'
 src=P('/mnt/media/ArcGIS_Linux/testdata/raster/'+source)
 shutil.copyfile(src,path)
 if os.path.exists(src+'.aux.xml'):shutil.copyfile(src+'.aux.xml',path+'.aux.xml')
 def digest(p):return hashlib.sha256(open(p,'rb').read()).hexdigest()
 before=digest(path)
 try:
  r=arcpy.management.SetRasterProperties(path,**kwargs)
  result=dict(case=label,source=source,kwargs=kwargs,status='succeeded',messages=r.getMessages())
  result['properties']={}
  for prop in ['SOURCETYPE','MINIMUM','MAXIMUM','MEAN','STD','ANYNODATA']:
   try: result['properties'][prop]=str(arcpy.management.GetRasterProperties(path,prop).getOutput(0))
   except Exception as query_error: result['properties'][prop]=dict(error=str(query_error))
 except Exception as e: result=dict(case=label,source=source,kwargs=kwargs,status='failed',messages=str(e))
 result['tiff_changed']=before!=digest(path)
 result['files']=sorted(os.listdir(folder))
 if os.path.exists(path+'.aux.xml'):result['aux']=open(path+'.aux.xml').read()
 print(json.dumps(result))
