"""Models for Slice: equal interval and equal area against ArcGIS's outputs."""
import numpy as np
from osgeo import gdal

gdal.UseExceptions()
D = "testdata/arcgis-reference/"


def load(path):
    ds = gdal.Open(path)
    b = ds.GetRasterBand(1)
    a = b.ReadAsArray().astype(np.float64)
    return a, b.GetNoDataValue()


def check(name, src, n, kind, base=1):
    a, nd = load(src)
    want, wnd = load(D + name + ".tif")
    valid = a != nd if nd is not None else np.ones(a.shape, bool)
    v = a[valid]
    lo, hi = v.min(), v.max()
    out = np.full(a.shape, -1.0)
    if kind == "EQUAL_INTERVAL":
        z = np.minimum(np.floor((a - lo) / ((hi - lo) / n)), n - 1)
        out[valid] = z[valid] + base
    else:
        # candidate: breakpoints at sorted-value quantiles of the float32 values
        s = np.sort(v)
        cuts = [s[int(len(s) * k / n)] for k in range(1, n)]
        z = np.searchsorted(cuts, a, side="right")
        out[valid] = z[valid] + base
    wv = want != wnd
    diff = int(((out != want) & (valid | wv)).sum())
    print(name, "cells differing:", diff, "of", int(valid.sum()))
    if diff and kind == "EQUAL_AREA":
        print("  want counts", [int((want == k).sum()) for k in range(base, base + n)])
        print("  ours counts", [int((out == k).sum()) for k in range(base, base + n)])


check("slice_ei5", "testdata/raster/dem.tif", 5, "EQUAL_INTERVAL")
check("slice_ei7_base3", "testdata/raster/dem.tif", 7, "EQUAL_INTERVAL", 3)
check("slice_ei3_nodata", "testdata/raster/dem_nodata.tif", 3, "EQUAL_INTERVAL")
check("slice_ei4_i16", "testdata/raster/patches_i16.tif", 4, "EQUAL_INTERVAL")
check("slice_ea5", "testdata/raster/dem.tif", 5, "EQUAL_AREA")
check("slice_ea4_nodata", "testdata/raster/dem_nodata.tif", 4, "EQUAL_AREA")
