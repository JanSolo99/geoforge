from osgeo import gdal
from pathlib import Path
gdal.UseExceptions()
Path('/tmp/unigis-u32-export-controls').mkdir(exist_ok=True)
src=gdal.Open('testdata/raster/composite_matrix/u32.tif')
for marker in [0,1,2147483647,2147483648,2147483649,4294967294,4294967295]:
 for cached in [False,True]:
  path=f'/tmp/unigis-u32-export-controls/nd_{marker}_{cached}.tif'
  out=gdal.GetDriverByName('GTiff').Create(path,src.RasterXSize,src.RasterYSize,1,gdal.GDT_UInt32)
  out.SetGeoTransform(src.GetGeoTransform());out.SetProjection(src.GetProjection())
  band=out.GetRasterBand(1);band.WriteArray(src.ReadAsArray());band.SetNoDataValue(marker)
  band=None;out=None
