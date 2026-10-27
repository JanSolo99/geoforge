"""Create private statistics-isolation fixtures (GDAL file IO only)."""
import argparse
from pathlib import Path
import numpy as np
from osgeo import gdal, osr

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--directory', type=Path, default=Path('/tmp'))
args = parser.parse_args()
gdal.UseExceptions()
args.directory.mkdir(parents=True, exist_ok=True)
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)
profiles = {
    'i8_narrow': (gdal.GDT_Int8, [-2, -1, 0, 1, 100, 126], None),
    'i16_narrow': (gdal.GDT_Int16, [-2, -1, 0, 1, 100, 200], None),
    'u16_narrow': (gdal.GDT_UInt16, [0, 1, 2, 127, 254, 325], None),
    'i8_wide': (gdal.GDT_Int8, [-128, -1, 0, 1, 126, 127], None),
    'i16_wide': (gdal.GDT_Int16, [-32768, -1, 0, 32767, 100, 200], None),
    'i16_ndmin': (gdal.GDT_Int16, [-32768, -1, 0, 32767, 100, 200], -32768),
}
for name, (pixel_type, values, nodata) in profiles.items():
    path = args.directory / ('unigis_unary_stats_base_' + name + '.tif')
    dataset = gdal.GetDriverByName('GTiff').Create(str(path), 3, 2, 1, pixel_type)
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(srs.ExportToWkt())
    band = dataset.GetRasterBand(1)
    band.WriteArray(np.asarray(values).reshape(2, 3))
    if nodata is not None:
        band.SetNoDataValue(nodata)
    band = None
    dataset = None
