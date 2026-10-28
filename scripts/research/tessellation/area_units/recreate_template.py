"""Create an empty EPSG2272 polygon shapefile as a file-format fixture.

Use a fresh private folder; inspect it before replacing committed fixture files.
OGR creates storage/SRS only. ArcGIS supplies all geoprocessing behavior.
"""
from pathlib import Path
import sys
from osgeo import ogr,osr
ogr.UseExceptions();osr.UseExceptions()
if len(sys.argv)!=2:raise SystemExit('Usage: recreate_template.py NEW_OUTPUT_FOLDER')
folder=Path(sys.argv[1]);folder.mkdir(parents=True,exist_ok=True)
path=folder/'feet_template.shp'
if path.exists():raise SystemExit('Choose a fresh output folder')
sr=osr.SpatialReference();sr.ImportFromEPSG(2272)
dataset=ogr.GetDriverByName('ESRI Shapefile').CreateDataSource(str(path))
layer=dataset.CreateLayer('feet_template',srs=sr,geom_type=ogr.wkbPolygon,options=['ENCODING=UTF-8'])
field=ogr.FieldDefn('Id',ogr.OFTInteger);field.SetWidth(6)
layer.CreateField(field)
layer=None;dataset=None

# Preserve the captured ArcGIS numeric precision; OGR's WKT exporter rounds it.
canonical=Path(__file__).with_name('spatial_reference.prj').read_text()
path.with_suffix('.prj').write_text(canonical)
