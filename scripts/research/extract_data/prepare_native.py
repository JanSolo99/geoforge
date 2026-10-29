"""Build disposable native outputs from the saved reference recipes, no ArcGIS writes."""
import json,subprocess
from pathlib import Path
root=Path('/mnt/media/ArcGIS_Linux')
folder=Path('/tmp/unigis-extract-native');folder.mkdir(exist_ok=False)
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
for name,case in manifest['cases'].items():
 if not name.startswith('edg_') or case['status']!='succeeded':continue
 out=folder/(name+'.gdb')
 params={k:str(v).replace('<out>',str(out)).replace('<folder>',str(folder)).replace('<name>',out.name) for k,v in case['run']['params'].items()}
 if params['out_type']=='GEODATABASE':
  subprocess.run([str(root/'target/debug/ug'),'run','create_table','output='+str(out/'temporary')],cwd=root,check=True)
  subprocess.run([str(root/'target/debug/ug'),'delete',str(out/'temporary')],cwd=root,check=True)
 subprocess.run([str(root/'target/debug/ug'),'run',case['run']['tool'],*[k+'='+v for k,v in params.items()]],cwd=root,check=True)
print('Native outputs saved',folder)
