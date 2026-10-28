"""Five embedded-cache regressions against saved ArcGIS file metadata."""
import subprocess,tempfile
from pathlib import Path
from osgeo import gdal
gdal.UseExceptions()
cases=[('expand_classes_dist_3','classes_u8',3,'1;2;5'),('expand_patches_dist_2_5','patches_i16',2,'2;7;12'),('expand_patches_dist_1_5','patches_i16',2,'-3;7'),('expand_zones_dist_12','zones',12,'7;0')]
failed=[]
with tempfile.TemporaryDirectory(prefix='unigis-cache-regressions-') as folder:
 for name,source,n,zones in cases+[('cpr_dem_nodata_in','dem_nodata',0,'')]:
  path=Path(folder)/(name+'.tif')
  args=['target/debug/ug','expand',f'testdata/raster/{source}.tif',str(path),str(n),zones,'--expand-method','DISTANCE'] if n else ['target/debug/ug','run','copy_raster',f'input=testdata/raster/{source}.tif',f'output={path}']
  subprocess.run(args,check=True,capture_output=True)
  ours=gdal.Open(str(path));ref=gdal.Open(f'testdata/arcgis-reference/{name}.tif')
  a=ours.GetRasterBand(1).GetMetadata();b=ref.GetRasterBand(1).GetMetadata();diff=[]
  for key,tol in [('COUNT',0),('MINIMUM',1e-5),('MAXIMUM',1e-5),('MEAN',1e-4),('STDDEV',1e-3)]:
   key='STATISTICS_'+key
   if key not in a or abs(float(a[key])-float(b[key]))>tol*max(1,abs(float(b[key]))):diff.append((key,a.get(key),b[key]))
  print(name,diff or 'PASS')
  if diff:failed.append(name)
raise SystemExit(bool(failed))
