"""Additional default/expansion/error controls against the immutable public source."""
import os,json
root=P('/tmp/unigis-extract-options');os.makedirs(root,exist_ok=False)
gdb=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/extract_base.gdb')
results={}
for name,items,opts in [
 ('table_default',['unrelated'],{}),('table_all',['unrelated'],dict(all_records_for_tables='ALL_RECORDS_FOR_TABLES')),
 ('no_related',['sites'],dict(get_related_data='DO_NOT_GET_RELATED')),
 ('schema_expanded',['sites'],dict(expand_feature_classes_and_tables='ADD_WITH_SCHEMA_ONLY')),
 ('no_expand_default',['sites'],dict(expand_feature_classes_and_tables='DO_NOT_ADD')),
 ('both_no_expand',['sites','observations'],dict(expand_feature_classes_and_tables='DO_NOT_ADD')),
 ('missing',['absent'],{}),('empty',[],{})]:
 args=dict(in_data=[os.path.join(gdb,n) for n in items],out_type='NEW_FILE_GEODATABASE',out_folder_path=root,out_name=name);args.update(opts)
 try:
  arcpy.management.ExtractDataFromGeodatabase(**args)
  dest=os.path.join(root,name+'.gdb');arcpy.env.workspace=dest
  results[name]=dict(status='succeeded',tables={n:list(arcpy.da.SearchCursor(os.path.join(dest,n),['OID@'])) for n in sorted(arcpy.ListFeatureClasses()+arcpy.ListTables())},relationships=[d.name for d in arcpy.Describe(dest).children if d.dataType=='RelationshipClass'],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-extract-options.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
