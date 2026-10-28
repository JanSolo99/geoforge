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
 if cached:arcpy.management.CalculateStatistics(P(base+f'nd_2147483649_{cached}.tif'),1,1)
 probe(2147483649,cached)
 probe(2147483648,cached,'equal',nodata_value='2147483648')
