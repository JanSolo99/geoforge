import tempfile,subprocess,json,itertools
from pathlib import Path
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
cases=[('expand_classes_dist_3','classes_u8',3,'1;2;5'),('expand_patches_dist_2_5','patches_i16',2,'2;7;12'),('expand_patches_dist_1_5','patches_i16',2,'-3;7'),('expand_zones_dist_12','zones',12,'7;0')]
with tempfile.TemporaryDirectory(prefix='unigis-expand-cache-') as folder:
 for name,source,n,zones in cases:
  src=gdal.Open(f'testdata/raster/{source}.tif');a=src.ReadAsArray();h,w=a.shape;nd=src.GetRasterBand(1).GetNoDataValue()
  print('source',source,{k:v for k,v in src.GetRasterBand(1).GetMetadata().items() if k in ['STATISTICS_COUNT','STATISTICS_MEAN','STATISTICS_STDDEV']},flush=True)
  input=Path(folder)/(name+'_in.tif');output=Path(folder)/(name+'_out.tif')
  d=gdal.GetDriverByName('GTiff').Create(str(input),w+2*n,h+2*n,1,src.GetRasterBand(1).DataType)
  d.SetGeoTransform(src.GetGeoTransform());d.SetProjection(src.GetProjection())
  b=d.GetRasterBand(1);b.SetNoDataValue(nd);b.WriteArray(np.pad(a,n,constant_values=nd));b=None;d=None
  subprocess.run(['target/debug/ug','expand',str(input),str(output),str(n),zones,'--expand-method','DISTANCE'],check=True,capture_output=True)
  d=gdal.Open(str(output));a=d.ReadAsArray().astype(float);nd=d.GetRasterBand(1).GetNoDataValue();valid=a!=nd
  meta=gdal.Open(f'testdata/arcgis-reference/{name}.tif').GetRasterBand(1) if False else None
  ref=gdal.Open(f'testdata/arcgis-reference/{name}.tif');m=ref.GetRasterBand(1).GetMetadata();wantn=int(float(m['STATISTICS_COUNT']));wants=round(float(m['STATISTICS_MEAN'])*wantn)
  def prefix(a):return np.pad(np.cumsum(np.cumsum(a,axis=0),axis=1),((1,0),(1,0)))
  counts=prefix(valid.astype(np.int64));sums=prefix(np.where(valid,a,0));squares=prefix(np.where(valid,a*a,0))
  def rect(p,l,r,t,b):
   x0=n-l;x1=n+w+r;y0=n-t;y1=n+h+b
   return p[y1,x1]-p[y0,x1]-p[y1,x0]+p[y0,x0]
  matches=[]
  for l,r,t,b in itertools.product(range(n+1),repeat=4):
   count=rect(counts,l,r,t,b);total=rect(sums,l,r,t,b)
   if count==wantn and total==wants:
    ss=rect(squares,l,r,t,b);std=((ss-total*total/count)/(count-1))**0.5
    if abs(std-float(m['STATISTICS_STDDEV']))<1e-8:matches.append([l,r,t,b])
  print(json.dumps({'case':name,'n':wantn,'sum':wants,'matches_LRTB':matches}),flush=True)
