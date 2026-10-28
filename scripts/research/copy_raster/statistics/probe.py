"""ArcGIS Pro black-box control: isolate source statistics from pixel contents."""
import json,os,xml.etree.ElementTree as ET
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/mnt/media/ArcGIS_Linux/testdata/raster/copy_statistics/'
def probe(profile,cached,case='default',**kwargs):
 src=base+profile+('_cached' if cached else '_uncached')+'.tif'
 out=f'/tmp/unigis_copy_stat_probe_{profile}_{cached}_{case}.tif'
 arcpy.management.CopyRaster(P(src),P(out),**kwargs)
 bands=[]
 aux=out+'.aux.xml'
 if os.path.exists(P(aux)):
  for band in ET.parse(P(aux)).getroot().findall('PAMRasterBand'):
   bands.append({'band':band.get('band'),'statistics':{n.get('key'):n.text for n in band.findall('Metadata/MDI') if n.get('key','').startswith('STATISTICS_')}})
 print(json.dumps({'input':profile,'cached':cached,'case':case,'bands':bands}))
for profile in ['large','small','multi','later','unused','large_valid']:
 for cached in [False,True]:probe(profile,cached)
for cached in [False,True]:
 for case,kwargs in [('f64',{'pixel_type':'64_BIT'}),('i16',{'pixel_type':'16_BIT_SIGNED'}),('override',{'nodata_value':'-9999'}),('same',{'pixel_type':'32_BIT_FLOAT'})]:
  probe('small',cached,case,**kwargs)
probe('small',True,'equal',nodata_value='-1.25')
probe('small',True,'equal_type',nodata_value='-1.25',pixel_type='32_BIT_FLOAT')
