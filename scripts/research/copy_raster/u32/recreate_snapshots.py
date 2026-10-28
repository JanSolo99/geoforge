"""Recreate checked-in source pixels with GDAL file IO; retain ArcGIS PAM caches."""
from pathlib import Path
from osgeo import gdal

gdal.UseExceptions()
folder=Path('testdata/raster/copy_u32')
src=gdal.Open('testdata/raster/composite_matrix/u32.tif')
profiles=[(f'nd_{marker}_{cache}',marker,cache=='cached')
          for marker in [0,1,2147483647,2147483648,2147483649,4294967294,4294967295]
          for cache in ['cached','uncached']]
profiles += [('neg_float_edge',4278190079,False),('neg_near_min',2147483775,False),('positive_edge',16777217,False)]
for name,marker,cached in profiles:
 path=folder/(name+'.tif');aux=Path(str(path)+'.aux.xml')
 original=aux.read_bytes() if cached else None
 out=gdal.GetDriverByName('GTiff').Create(str(path),3,2,1,gdal.GDT_UInt32)
 out.SetGeoTransform(src.GetGeoTransform());out.SetProjection(src.GetProjection())
 band=out.GetRasterBand(1);band.WriteArray(src.ReadAsArray());band.SetNoDataValue(marker)
 band=None;out=None
 if cached:aux.write_bytes(original)
 elif aux.exists():raise RuntimeError(f'unexpected statistics: {aux}')
