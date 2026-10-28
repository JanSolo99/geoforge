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
arcpy.management.CalculateStatistics(P(base+'cached_u8.tif'),1,1)
for marker in [126.75,127.25]:
 run(base+'cached_u8.tif','8_BIT_UNSIGNED',marker,'u8_cached_'+str(marker))
for marker in [255.25,-0.25]:
 run('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/nd_u8.tif','8_BIT_UNSIGNED',marker,'u8_edge_'+str(marker))
