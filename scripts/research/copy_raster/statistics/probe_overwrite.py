import json,os,xml.etree.ElementTree as ET
arcpy.env.overwriteOutput=True
out='/tmp/unigis_copy_overwrite_oracle.tif'
for src in ['/mnt/media/ArcGIS_Linux/testdata/raster/dem.tif','/mnt/media/ArcGIS_Linux/testdata/raster/copy_statistics/small_cached.tif']:
 arcpy.management.CopyRaster(P(src),P(out))
 root=ET.parse(P(out+'.aux.xml')).getroot() if os.path.exists(P(out+'.aux.xml')) else None
 stats={n.get('key'):n.text for n in root.findall('.//MDI') if n.get('key','').startswith('STATISTICS_')} if root is not None else {}
 print(json.dumps({'input':src,'statistics':stats}))
