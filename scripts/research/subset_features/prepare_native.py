"""Replay saved successful partitions into distinct native datasets."""
from pathlib import Path
import json,subprocess
root=Path('/mnt/media/ArcGIS_Linux');folder=Path('/tmp/unigis-subset-native');folder.mkdir(exist_ok=False)
for name,case in json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())['cases'].items():
 if not name.startswith('subf_') or case['status']!='succeeded':continue
 out=folder/name;out.mkdir()
 params={k:str(v).replace('<out>',str(out)) for k,v in case['run']['params'].items()}
 subprocess.run([str(root/'target/debug/ug'),'run',case['run']['tool'],*[k+'='+v for k,v in params.items()]],cwd=root,check=True)
gdb=folder/'outputs.gdb'
subprocess.run([str(root/'target/debug/ug'),'run','create_table','output='+str(gdb/'disposable')],cwd=root,check=True)
subprocess.run([str(root/'target/debug/ug'),'delete',str(gdb/'disposable')],cwd=root,check=True)
for item in ['points','records']:
 subprocess.run([str(root/'target/debug/ug'),'run','subset_features','input='+str(root/'testdata/geoproc/subset_base.gdb'/item),'out_training_feature_class='+str(gdb/(item+'_train')),'out_test_feature_class='+str(gdb/(item+'_test'))],cwd=root,check=True)
for name,source,extension in [('null_control','/tmp/unigis-subset-nulls/control.gdb/nullable','shp'),('null_dates','/tmp/unigis-subset-null-dates/source.gdb/dates','dbf')]:
 subprocess.run([str(root/'target/debug/ug'),'run','subset_features','input='+source,'out_training_feature_class='+str(folder/(name+'.'+extension)),'size_of_training_dataset=100'],cwd=root,check=True)
print('Native partitions saved',folder)
