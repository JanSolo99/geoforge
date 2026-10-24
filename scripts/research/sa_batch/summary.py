"""Type, grid, NoData and value summary of the reference outputs: inspect.py name..."""
import sys

import numpy as np
from osgeo import gdal

gdal.UseExceptions()
D = "testdata/arcgis-reference/"
for n in sys.argv[1:]:
    ds = gdal.Open(D + n + ".tif")
    b = ds.GetRasterBand(1)
    a = b.ReadAsArray()
    nd = b.GetNoDataValue()
    v = a[a != nd] if nd is not None else a.ravel()
    u = np.unique(v)
    print(n, gdal.GetDataTypeName(b.DataType), ds.RasterXSize, ds.RasterYSize, ds.GetGeoTransform()[:1] + ds.GetGeoTransform()[3:4],
          "nodata", nd, "n", v.size, "uniq", u[:12], len(u),
          "counts", [int((v == x).sum()) for x in u[:12]] if len(u) <= 12 else "")
