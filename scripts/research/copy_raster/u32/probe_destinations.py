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
for cached in [False,True]:
 probe(2147483648,cached,'f32',pixel_type='32_BIT_FLOAT')
 probe(2147483648,cached,'i16',pixel_type='16_BIT_SIGNED')
probe(2147483649,False,'f64',pixel_type='64_BIT')
probe(2147483649,False,'i32',pixel_type='32_BIT_SIGNED')
