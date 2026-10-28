import json,os,xml.etree.ElementTree as ET
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-u32-export-controls/'
def probe(marker,cached,name='default',**kwargs):
 src=base+f'nd_{marker}_{cached}.tif'
 out=base+f'result_{marker}_{cached}_{name}.tif'
 arcpy.management.CopyRaster(P(src),P(out),**kwargs)
 raster=arcpy.Raster(P(out))
 stats={}
 if os.path.exists(P(out+'.aux.xml')):
  root=ET.parse(P(out+'.aux.xml')).getroot()
  stats={n.get('key'):n.text for n in root.findall('.//MDI') if n.get('key','').startswith('STATISTICS_')}
 print(json.dumps({'marker':marker,'cached':cached,'case':name,'pixel_type':raster.pixelType,'nodata':raster.noDataValue,'values':arcpy.RasterToNumPyArray(raster).tolist(),'statistics':stats}))
for marker in [0,1,2147483647,2147483648,4294967294,4294967295]:
 for cached in [False,True]:
  if cached:arcpy.management.CalculateStatistics(P(base+f'nd_{marker}_{cached}.tif'),1,1)
  probe(marker,cached)
for cached in [False,True]:
 for name,kwargs in [('same',{'pixel_type':'32_BIT_UNSIGNED'}),('override_low',{'nodata_value':'1000000000'}),('override_high',{'nodata_value':'4294967294'}),('f64',{'pixel_type':'64_BIT'}),('i32',{'pixel_type':'32_BIT_SIGNED'})]:
  probe(2147483648,cached,name,**kwargs)
