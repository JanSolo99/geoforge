"""Writes the test rasters in testdata/raster/ with GDAL (test-data generation only;
GeoForge reads them without GDAL): a synthetic DEM in UTM 33N in several GeoTIFF
encodings, plus small integer rasters.

    python3 scripts/make-raster-data.py
"""

from pathlib import Path

import numpy as np
from osgeo import gdal, osr

OUT = Path(__file__).resolve().parent.parent / "testdata" / "raster"
OUT.mkdir(parents=True, exist_ok=True)
gdal.UseExceptions()


def dem(rows=120, cols=150):
    y, x = np.mgrid[0:rows, 0:cols].astype(np.float64)
    z = 200.0
    for cx, cy, h, s in [(40, 30, 120, 18), (110, 80, 90, 25), (75, 100, 60, 12)]:
        z = z + h * np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * s * s))
    z += 0.15 * x - 0.1 * y + 3 * np.sin(x / 7.0) * np.cos(y / 9.0)
    return z


def write(name, data, dtype, nodata=None, options=(), cell=10.0, point=False):
    if data.ndim == 2:
        data = data[np.newaxis]
    bands, rows, cols = data.shape
    drv = gdal.GetDriverByName("GTiff")
    ds = drv.Create(str(OUT / name), cols, rows, bands, dtype, list(options))
    if point:
        ds.SetMetadataItem("AREA_OR_POINT", "Point")
    ds.SetGeoTransform((500000.0, cell, 0.0, 5001200.0, 0.0, -cell))
    sr = osr.SpatialReference()
    sr.ImportFromEPSG(32633)
    ds.SetProjection(sr.ExportToWkt())
    for i in range(bands):
        band = ds.GetRasterBand(i + 1)
        if nodata is not None:
            band.SetNoDataValue(nodata)
        band.WriteArray(data[i])
    ds = None


z = dem()
z32 = z.astype(np.float32)
write("dem.tif", z32, gdal.GDT_Float32)
write("dem_tiled_deflate.tif", z32, gdal.GDT_Float32, options=["TILED=YES", "BLOCKXSIZE=64", "BLOCKYSIZE=32", "COMPRESS=DEFLATE", "PREDICTOR=3"])
write("dem_lzw.tif", z32, gdal.GDT_Float32, options=["COMPRESS=LZW", "PREDICTOR=2"])
holes = z32.copy()
holes[10:20, 10:25] = -9999.0
holes[60, :] = -9999.0
write("dem_nodata.tif", holes, gdal.GDT_Float32, nodata=-9999.0)
write("dem_int16_packbits.tif", np.round(z).astype(np.int16), gdal.GDT_Int16, options=["COMPRESS=PACKBITS"])
landuse = ((np.indices((40, 50)).sum(axis=0) // 7) % 5).astype(np.uint8)
write("landuse_u8.tif", landuse, gdal.GDT_Byte, nodata=255, options=["TILED=YES", "BLOCKXSIZE=16", "BLOCKYSIZE=16", "COMPRESS=LZW"], cell=30.0)
write("dem_f64.tif", z, gdal.GDT_Float64)
write("dem_bigendian.tif", z32, gdal.GDT_Float32, options=["ENDIANNESS=BIG", "TILED=YES", "COMPRESS=DEFLATE", "PREDICTOR=3"])
write("dem_bigtiff_i32.tif", np.round(z * 100).astype(np.int32), gdal.GDT_Int32, options=["BIGTIFF=YES", "COMPRESS=LZW", "PREDICTOR=2", "ENDIANNESS=BIG"])
write("dem_point.tif", z32, gdal.GDT_Float32, point=True)
rgb = np.stack([np.round(z).astype(np.uint16), np.round(z * 2).astype(np.uint16), (np.arange(z.size).reshape(z.shape) % 65536).astype(np.uint16)])
write("rgb_planar_u16.tif", rgb, gdal.GDT_UInt16, options=["INTERLEAVE=BAND", "COMPRESS=LZW", "PREDICTOR=2"])
write("rgb_pixel_u8.tif", (rgb % 256).astype(np.uint8), gdal.GDT_Byte, options=["INTERLEAVE=PIXEL", "TILED=YES", "COMPRESS=DEFLATE", "PREDICTOR=2"])
# Hydrology: a valley draining south with single-cell pits, a closed basin, a
# flat terrace and a NoData hole.
y, x = np.mgrid[0:80, 0:100].astype(np.float64)
hydro = 100 + 0.004 * (x - 50) ** 2 - 0.3 * y + 1.5 * np.sin(x / 5.0) * np.cos(y / 6.0)
hydro += 0.01 * np.sin(x * 1.7 + y * 0.9)
for py, px in [(20, 30), (40, 60), (55, 48), (10, 80)]:
    hydro[py, px] -= 4.0
hydro[30:38, 15:25] -= 6.0 * np.exp(-((y[30:38, 15:25] - 34) ** 2 + (x[30:38, 15:25] - 20) ** 2) / 8)
hydro[60:66, 70:80] = 90.0
hydro = hydro.astype(np.float32)
hydro_nd = hydro.copy()
hydro_nd[5:8, 40:43] = -9999.0
write("hydro.tif", hydro, gdal.GDT_Float32)
write("hydro_nodata.tif", hydro_nd, gdal.GDT_Float32, nodata=-9999.0)
write("hydro_int.tif", np.round(hydro).astype(np.int16), gdal.GDT_Int16)
# Zones on the DEM grid: six irregular zones, one split in two parts, and a
# NoData corner.
yy, xx = np.mgrid[0:120, 0:150]
zones = ((xx // 50) + 3 * (yy // 60)).astype(np.uint8)
zones[(xx - 75) ** 2 + (yy - 60) ** 2 < 15 ** 2] = 7
zones[100:, 130:] = 255
write("zones.tif", zones, gdal.GDT_Byte, nodata=255)
