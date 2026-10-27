"""Create small typed frequency inputs with GDAL for file IO; ArcGIS is the GP oracle."""
from pathlib import Path
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
folder = Path('testdata/raster/frequency')
folder.mkdir(exist_ok=True)
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
a = [0, -9999, 2, 3, 4, 5]
b = [5, 4, 3, 2, -9999, 0]
c = [0, 2, -9999, 3, 5, 4]
d = [0, 1, 3, -9999, 4, 0]
for name, bands in {'value2': [a, b], 'list2': [c, d], 'single': [a], 'three': [c, d, a]}.items():
    dataset = gdal.GetDriverByName('GTiff').Create(str(folder / (name + '.tif')), 3, 2, len(bands), gdal.GDT_Float32)
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    for i, values in enumerate(bands, 1):
        band = dataset.GetRasterBand(i)
        band.WriteArray(np.asarray(values, dtype=np.float32).reshape(2, 3))
        band.SetNoDataValue(-9999)
    band = None
    dataset = None
