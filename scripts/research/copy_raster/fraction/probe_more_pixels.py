import json,os,xml.etree.ElementTree as ET
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-copy-fraction-controls/'
def run(source,pixel,marker,label):
 out=base+label+'.tif';kwargs={'pixel_type':pixel}
 if marker is not None:kwargs['nodata_value']=str(marker)
 arcpy.management.CopyRaster(P(source),P(out),**kwargs)
 r=arcpy.Raster(P(out));stats={}
 if os.path.exists(P(out+'.aux.xml')):
  root=ET.parse(P(out+'.aux.xml')).getroot()
  stats={v.get('key'):v.text for v in root.findall('.//MDI') if v.get('key','').startswith('STATISTICS_')}
 print(json.dumps({'case':label,'argument':marker,'source':source,'pixel_type':r.pixelType,'nodata':r.noDataValue,'cells':arcpy.RasterToNumPyArray(r).tolist(),'statistics':stats}))
run(base+'pixel_quarters.tif','16_BIT_SIGNED',None,'pixel_quarters_i16')
run(base+'payload_tie.tif','16_BIT_SIGNED','3.4e38','payload_tie_i16')
