"""Save disposable native outputs using the reference registry recipes."""
from pathlib import Path
import json,subprocess
root=Path('/mnt/media/ArcGIS_Linux');folder=Path('/tmp/unigis-spatial-sampling-native-v2');folder.mkdir(exist_ok=False)
for name,case in json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())['cases'].items():
 if not name.startswith('ssl_') or case['status']!='succeeded':continue
 params={k:str(v).replace('<out>',str(folder/(name+'.shp'))) for k,v in case['run']['params'].items()}
 subprocess.run([str(root/'target/debug/ug'),'run',case['run']['tool'],*[k+'='+v for k,v in params.items()]],cwd=root,check=True)
print('Native outputs saved',folder)

subprocess.run([str(root/'target/debug/ug'),'run','create_spatial_sampling_locations',
    'input='+str(root/'testdata/geoproc/ssl_areas.shp'),'output='+str(folder/'within_small.shp'),
    'sampling_method=SYSTEMATIC','bin_shape=SQUARE','bin_size=25 SquareMeters',
    'geometry_type=POLYGON','spatial_relationship=COMPLETELY_WITHIN'],cwd=root,check=True)
