"""Recreate private tiny/cached/F64-extreme inputs using GDAL for file IO."""
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
values = [.1, -1.25, 32768.5, 16777217.25, 4000000000.125, 3.5]
profiles = {
    'i16_tiny': (gdal.GDT_Int16, [0, 1, 2, 3, 4, 5], None),
    'i16_bit': (gdal.GDT_Int16, [0, 1, 0, 1, 0, 1], None),
    'i16_negative': (gdal.GDT_Int16, [-5, -4, -3, -2, -1, 0], None),
    'f64_cached': (gdal.GDT_Float64, values, None),
    'f64_nd_cached': (gdal.GDT_Float64, values, -1.25),
    'f64_extreme': (gdal.GDT_Float64, [.1, -.1, 1e200, -1e200, 1e308, -1e308], None),
}
for name, (pixel_type, cells, nodata) in profiles.items():
    dataset = gdal.GetDriverByName('GTiff').Create('/tmp/unigis_unary_extra_' + name + '.tif', 3, 2, 1, pixel_type)
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    band = dataset.GetRasterBand(1)
    band.WriteArray(np.asarray(cells).reshape(2, 3))
    if nodata is not None:
        band.SetNoDataValue(nodata)
    band = None
    dataset = None
