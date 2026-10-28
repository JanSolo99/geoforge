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
for marker in [-32768.25,-32768.5,-32768.75,32767.25,32767.5,32767.75]:
 run('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/nd_f32.tif','16_BIT_SIGNED',marker,'i16_edge_'+str(marker))
for marker in [-0.0,0.0]:
 run('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/nd_u8.tif','8_BIT_UNSIGNED',marker,'u8_zero_'+str(marker))
