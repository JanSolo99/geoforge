"""Write explicitly typed test inputs with GDAL, then verify every stored value.

GDAL is used solely for file IO. ArcGIS supplies the operation reference outputs.
Run from the repository root with GDAL's Python bindings and NumPy installed.
Do not use AAIGrid inference: it can narrow integer or double boundary values.
"""
import argparse
import json
from pathlib import Path

import numpy as np
from osgeo import gdal, osr

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--check', action='store_true', help='Verify existing inputs without rewriting files.')
args = parser.parse_args()
gdal.UseExceptions()
values = json.loads(Path(__file__).with_name('matrix_inputs.json').read_text())
folder = Path('testdata/raster/composite_matrix')
folder.mkdir(parents=True, exist_ok=True)
types = {
    'u8': (gdal.GDT_Byte, np.uint8), 'i8': (gdal.GDT_Int8, np.int8),
    'u16': (gdal.GDT_UInt16, np.uint16), 'i16': (gdal.GDT_Int16, np.int16),
    'u32': (gdal.GDT_UInt32, np.uint32), 'i32': (gdal.GDT_Int32, np.int32),
    'f32': (gdal.GDT_Float32, np.float32), 'f64': (gdal.GDT_Float64, np.float64),
}
no_data = {'u8': 127, 'i8': -1, 'u16': 32767, 'i16': -1, 'u32': 2147483648,
           'i32': -1, 'f32': -1.25, 'f64': -1.25}
srs = osr.SpatialReference()
srs.ImportFromEPSG(32633)

def write(name, kind, x=500000, nodata=None):
    dtype, numpy_type = types[kind]
    array = np.asarray(values[kind][1], dtype=numpy_type).reshape(2, 3)
    path = folder / (name + '.tif')
    if not args.check:
        ds = gdal.GetDriverByName('GTiff').Create(str(path), 3, 2, 1, dtype)
        ds.SetGeoTransform((x, 10, 0, 5000020, 0, -10))
        ds.SetProjection(srs.ExportToWkt())
        band = ds.GetRasterBand(1)
        band.WriteArray(array)
        if nodata is not None:
            band.SetNoDataValue(nodata)
        band = None
        ds = None
    check = gdal.Open(str(path))
    assert check.GetGeoTransform() == (x, 10, 0, 5000020, 0, -10)
    assert check.GetSpatialRef().GetAuthorityCode(None) == '32633'
    assert check.GetRasterBand(1).DataType == dtype
    assert check.GetRasterBand(1).GetNoDataValue() == nodata
    np.testing.assert_array_equal(check.ReadAsArray(), array, err_msg=str(path))
    check = None

for kind in values:
    write(kind, kind)
    write('nd_' + kind, kind, nodata=no_data[kind])
write('shifted', 'u8', x=500010)
write('shifted_nd_u8', 'u8', x=500010, nodata=127)
print('Verified all stored values, grids, pixel types and NoData tags in', 2 * len(values) + 2, 'fixtures.')
