"""Probe the dynamic default for related data under ADD_WITH_SCHEMA_ONLY."""
import os,json
root=P('/tmp/unigis-extract-defaults');os.makedirs(root,exist_ok=False)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/extract_base.gdb/sites')
results={}
for name,extra in [('omitted',{}),('explicit_get',dict(get_related_data='GET_RELATED')),('explicit_no',dict(get_related_data='DO_NOT_GET_RELATED'))]:
 arcpy.management.ExtractDataFromGeodatabase(source,out_type='NEW_FILE_GEODATABASE',out_folder_path=root,out_name=name,expand_feature_classes_and_tables='ADD_WITH_SCHEMA_ONLY',**extra)
 results[name]={n:int(arcpy.management.GetCount(os.path.join(root,name+'.gdb',n)).getOutput(0)) for n in ['sites','observations']}
with open(P('/tmp/unigis-extract-defaults.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
