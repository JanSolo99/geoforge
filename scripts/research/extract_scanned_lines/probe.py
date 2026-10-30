"""Extract Scanned Lines probes (run through the ArcGIS bridge's arcgis_python, ArcGIS Pro 3.7).

Synthetic 1-bit scans: a cross of a 5 px horizontal and a 3 px vertical bar, and a slanted
4 px line touching a 3 px ring. Results are in README.md.
"""
import os
import shutil

import numpy as np

base = P('/tmp/ugc/esl')
shutil.rmtree(base, ignore_errors=True)
os.makedirs(base)
a = np.zeros((60, 80), dtype=np.uint8)
a[28:33, 5:75] = 1
a[5:55, 38:41] = 1
arcpy.NumPyArrayToRaster(a, arcpy.Point(0, 0), 1, 1).save(base + '\\cross.tif')
arcpy.conversion.ExtractScannedLines(base + '\\cross.tif', 1, base + '\\c.shp')
print([r for r in arcpy.da.SearchCursor(base + '\\c.shp', ['OID@', 'SHAPE@WKT', 'Width'])])

a = np.zeros((80, 80), dtype=np.uint8)
yy, xx = np.mgrid[0:80, 0:80]
a[np.abs((xx - 10) - (yy - 10) * 0.6) < 2] = 1
a[np.abs(np.hypot(xx - 50, yy - 40) - 18) < 1.6] = 1
arcpy.NumPyArrayToRaster(a, arcpy.Point(0, 0), 1, 1).save(base + '\\curve.tif')
for kw in [{}, {'smoothing': False}, {'smoothing': False, 'compression': 0.001}]:
    o = base + '\\k%d.shp' % len(kw)
    arcpy.conversion.ExtractScannedLines(base + '\\curve.tif', 1, o, **kw)
    print(kw, [r for r in arcpy.da.SearchCursor(o, ['OID@', 'SHAPE@WKT', 'Width'])])
