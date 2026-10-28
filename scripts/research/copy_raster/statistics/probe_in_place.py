import shutil,json,hashlib,os
arcpy.env.overwriteOutput=True
src='/mnt/media/ArcGIS_Linux/testdata/raster/copy_statistics/small_cached.tif'
target='/tmp/unigis_copy_in_place_oracle.tif'
shutil.copyfile(P(src),P(target))
shutil.copyfile(P(src+'.aux.xml'),P(target+'.aux.xml'))
def digest(path):return hashlib.sha256(open(P(path),'rb').read()).hexdigest()
before={p:digest(p) for p in [target,target+'.aux.xml']}
try:
 arcpy.management.CopyRaster(P(target),P(target))
 print(json.dumps({'status':'success','preserved':all(digest(p)==v for p,v in before.items())}))
except Exception as e:
 print(json.dumps({'status':'error','message':str(e),'preserved':all(digest(p)==v for p,v in before.items())}))
