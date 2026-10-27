"""Recreate cached probe pixels with GDAL file IO, retaining ArcGIS sidecars.

The checked-in aux.xml files are ArcGIS CalculateStatistics outputs. This script
never uses GDAL's statistics as a geoprocessing oracle.
"""
import json
from pathlib import Path
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
folder = Path('testdata/raster/unary_cached')
profiles = json.loads(Path(__file__).with_name('cached_inputs.json').read_text())
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
for name, profile in profiles.items():
    path = folder / (name + '.tif')
    sidecar = Path(str(path) + '.aux.xml')
    statistics = sidecar.read_bytes()
    dataset = gdal.GetDriverByName('GTiff').Create(str(path), 3, 2, 1, gdal.GetDataTypeByName(profile['type']))
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    band = dataset.GetRasterBand(1)
    band.WriteArray(np.asarray(profile['values']))
    if profile['nodata'] is not None:
        band.SetNoDataValue(profile['nodata'])
    band = None
    dataset = None
    sidecar.write_bytes(statistics)
