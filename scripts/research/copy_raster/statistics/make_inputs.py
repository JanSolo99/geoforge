"""Pixel-only fixture creation with GDAL file IO, retaining real ArcGIS PAM stats.

Run at the repository root. The six cached sidecars are saved CalculateStatistics
outputs, never GDAL-computed statistics. No source statistics are copied.
"""
from pathlib import Path
from osgeo import gdal

gdal.UseExceptions()
folder=Path('testdata/raster/copy_statistics')
folder.mkdir(exist_ok=True)
profiles={
 'large':'testdata/raster/dem_nodata.tif',
 'small':'testdata/raster/composite_matrix/nd_f32.tif',
 'multi':'testdata/raster/copy_raster/first_nodata.tif',
 'later':'testdata/raster/copy_raster/later_nodata.tif',
 'unused':'testdata/raster/composite_matrix/f32.tif',
 'large_valid':'testdata/raster/dem.tif',
}
for name,source in profiles.items():
 src=gdal.Open(source)
 for cached in [False,True]:
  path=folder/f'{name}_{"cached" if cached else "uncached"}.tif'
  aux=Path(str(path)+'.aux.xml')
  original=aux.read_bytes() if cached else None
  out=gdal.GetDriverByName('GTiff').Create(str(path),src.RasterXSize,src.RasterYSize,src.RasterCount,src.GetRasterBand(1).DataType)
  out.SetGeoTransform(src.GetGeoTransform());out.SetProjection(src.GetProjection())
  for i in range(1,src.RasterCount+1):
   band=out.GetRasterBand(i)
   band.WriteArray(src.GetRasterBand(i).ReadAsArray())
   nodata=-9999.0 if name=='unused' else src.GetRasterBand(i).GetNoDataValue()
   if nodata is not None:band.SetNoDataValue(nodata)
   band=None
  out=None
  if cached:aux.write_bytes(original)
  elif aux.exists():raise RuntimeError(f'unexpected statistics sidecar: {aux}')
