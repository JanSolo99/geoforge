"""Replay the saved successful recipes with native ug into disposable outputs."""
from pathlib import Path
import json,subprocess
root=Path('/mnt/media/ArcGIS_Linux');folder=Path('/tmp/unigis-random-native');folder.mkdir(exist_ok=False)
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
for name,case in manifest['cases'].items():
 if not name.startswith('rnp_') or case['status']!='succeeded':continue
 params={k:str(v).replace('<out>',str(folder/(name+'.shp'))) for k,v in case['run']['params'].items()}
 subprocess.run([str(root/'target/debug/ug'),'run',case['run']['tool'],*[k+'='+v for k,v in params.items()]],cwd=root,check=True)
print('Native outputs saved',folder)
