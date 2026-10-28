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
for marker in [-9999.25,-9999.5,-9999.75,0.5,1.5,-0.5,-1.5]:
 run('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/nd_f32.tif','16_BIT_SIGNED',marker,'i16_arg_'+str(marker))
for marker in [126.25,126.5,126.75,127.5,128.5,255.5,300.25,300.5,300.75,-0.5]:
 run('/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/nd_u8.tif','8_BIT_UNSIGNED',marker,'u8_arg_'+str(marker))
for marker in [-1.5,-1.75,1.5,1.75,0.5]:
 source=base+'source_'+str(marker)+'.tif'
 run(source,'16_BIT_SIGNED',None,'i16_default_'+str(marker))
 run(source,'16_BIT_SIGNED',marker,'i16_equal_'+str(marker))
