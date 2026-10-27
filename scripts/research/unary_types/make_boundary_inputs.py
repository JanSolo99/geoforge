"""Create private integer boundary inputs with GDAL for file IO only."""
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
profiles = {
    'square_i32': (gdal.GDT_Int32, [46340, 46341, -46340, -46341, 2147483647, -2147483648]),
    'square_u32': (gdal.GDT_UInt32, [0, 1, 65535, 65536, 4294967295, 65534]),
    'negate_u32': (gdal.GDT_UInt32, [0, 2147483647, 2147483648, 2147483649, 4294967294, 4294967295]),
}
for name, (pixel_type, values) in profiles.items():
    dataset = gdal.GetDriverByName('GTiff').Create('/tmp/unigis_unary_boundary_' + name + '.tif', 3, 2, 1, pixel_type)
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    band = dataset.GetRasterBand(1)
    band.WriteArray(np.asarray(values).reshape(2, 3))
    band = None
    dataset = None
