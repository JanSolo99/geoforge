"""Recreate the F32 Log2 precision file-format fixture in a fresh directory."""
from pathlib import Path
import json,sys
import numpy as np
from osgeo import gdal,osr
gdal.UseExceptions();osr.UseExceptions()
if len(sys.argv)!=2:raise SystemExit('Usage: recreate_precision.py NEW_OUTPUT_DIRECTORY')
folder=Path(sys.argv[1]);folder.mkdir(parents=True,exist_ok=True);path=folder/'input.tif'
if path.exists():raise SystemExit('Choose a fresh output directory')
values=np.asarray(json.loads(Path(__file__).with_name('precision_values.json').read_text()),dtype=np.float32).reshape(4,8)
sr=osr.SpatialReference();sr.ImportFromEPSG(32633)
ds=gdal.GetDriverByName('GTiff').Create(str(path),8,4,2,gdal.GDT_Float32)
ds.SetGeoTransform((500000,10,0,5000040,0,-10));ds.SetProjection(sr.ExportToWkt())
for i in [1,2]:
 band=ds.GetRasterBand(i);band.WriteArray(values);band.SetNoDataValue(-9999)
band=None;ds=None
ds=gdal.Open(str(path))
for i in [1,2]:assert np.array_equal(ds.GetRasterBand(i).ReadAsArray(),values)
print('Precision fixture read-back verified')
