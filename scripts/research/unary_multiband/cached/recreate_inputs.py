"""Recreate cached integer multiband TIFFs with frozen ArcGIS statistics.

GDAL supplies file IO only. ArcGIS is the geoprocessing behavior oracle.
Pass a fresh destination directory; existing files are never overwritten.
"""
from pathlib import Path
import json
import sys
import shutil
import numpy as np
from osgeo import gdal, osr

gdal.UseExceptions()
osr.UseExceptions()
if len(sys.argv) != 2:
    raise SystemExit("Usage: recreate_inputs.py NEW_OUTPUT_DIRECTORY")
folder = Path(sys.argv[1])
profiles = json.loads(Path(__file__).with_name("profiles.json").read_text())
if any((folder / (name + ".tif")).exists() for name in profiles):
    raise SystemExit("Choose a fresh output directory")
folder.mkdir(parents=True, exist_ok=True)
sr = osr.SpatialReference()
sr.ImportFromEPSG(32633)
for name, profile in profiles.items():
    path = folder / (name + ".tif")
    dataset = gdal.GetDriverByName("GTiff").Create(
        str(path), 3, 2, len(profile["bands"]), gdal.GetDataTypeByName(profile["type"])
    )
    dataset.SetGeoTransform((500000, 10, 0, 5000020, 0, -10))
    dataset.SetProjection(sr.ExportToWkt())
    for index, values in enumerate(profile["bands"], 1):
        band = dataset.GetRasterBand(index)
        band.WriteArray(np.asarray(values))
        band.SetNoDataValue(profile["nodata"])
    band = None
    dataset = None
    dataset = gdal.Open(str(path))
    for index, values in enumerate(profile["bands"], 1):
        band = dataset.GetRasterBand(index)
        assert band.DataType == gdal.GetDataTypeByName(profile["type"])
        assert np.array_equal(band.ReadAsArray(), np.asarray(values))
        assert band.GetNoDataValue() == profile["nodata"]
        assert not any(key.startswith("STATISTICS_") for key in band.GetMetadata())
    band = None
    dataset = None
    shutil.copyfile(Path(__file__).with_name('cache') / (name + '.tif.aux.xml'), Path(str(path) + '.aux.xml'))
print(f"Created and read-back verified {len(profiles)} cached fixtures")
