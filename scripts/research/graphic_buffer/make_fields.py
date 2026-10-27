"""Create synthetic Graphic Buffer inputs. GDAL writes files; ArcGIS is the behavior oracle."""
import json
from pathlib import Path
import subprocess
import tempfile

root = Path.cwd()
out = root / 'testdata/geoproc/graphic_buffer_fields.shp'
rows = [
    (10., '10 Meters', 10., '10 Blargs', 10.),
    (20., '0.02 Kilometers', 0., 'garbage', None),
    (30., '30 Feet', -5., '', 30.),
]
features = []
for i, (dist, text, mixed, bad, nullable) in enumerate(rows):
    features.append({'type': 'Feature', 'geometry': {'type': 'Point', 'coordinates': [500000 + i * 300, 5000000]},
                     'properties': {'CASE_ID': i + 1, 'DIST': dist, 'TEXT_DIST': text, 'ZERO': 0., 'NEG': -5.,
                                    'MIX': mixed, 'TEXT_MIX': ['10 Meters','0 Meters','20 Meters'][i], 'TEXT_NULL': ['10 Meters',None,'30 Meters'][i], 'TEXT_NEG': '-5 Meters', 'BAD': bad, 'NULL_DIST': nullable, 'UNKNOWN': '10 Blargs'}})
with tempfile.TemporaryDirectory() as tmp:
    source = Path(tmp) / 'fields.geojson'
    source.write_text(json.dumps({'type': 'FeatureCollection', 'features': features}))
    subprocess.run(['ogr2ogr', '-overwrite', '-f', 'ESRI Shapefile', str(out), str(source), '-a_srs', 'EPSG:32633'], check=True)
