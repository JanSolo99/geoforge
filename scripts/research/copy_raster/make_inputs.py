"""Create tiny multi-band fixtures with GDAL for file IO only; ArcGIS is the behavior oracle."""
from pathlib import Path
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
folder = Path('testdata/raster/copy_raster')
folder.mkdir(exist_ok=True)
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
for name in ['first', 'later']:
    dataset = gdal.GetDriverByName('GTiff').Create(str(folder / (name + '_nodata.tif')), 2, 2, 2, gdal.GDT_Float32)
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    for index in [1, 2]:
        band = dataset.GetRasterBand(index)
        cells = [[1, -9999], [3, 4]] if (index == 1) == (name == 'first') else [[1, 2], [3, 4]]
        band.WriteArray(np.asarray(cells, dtype=np.float32))
        band.SetNoDataValue(-9999)
    band = None
    dataset = None
