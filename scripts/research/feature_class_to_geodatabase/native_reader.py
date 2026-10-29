"""Public ArcGIS reads native imports and compares them with saved public outputs."""
import os,json,math
with open(P('/tmp/unigis-fctg-native-config.json')) as f:config=json.load(f)
root=P('/mnt/media/ArcGIS_Linux/testdata/arcgis-reference')
def snapshot(gdb):
 arcpy.env.workspace=gdb
 items=list(arcpy.ListFeatureClasses() or [])
 for ds in arcpy.ListDatasets('','Feature') or []:
  items.extend(ds+'/'+n for n in arcpy.ListFeatureClasses(feature_dataset=ds) or [])
 out={}
 for name in sorted(items):
  path=gdb+'/'+name;fs=arcpy.ListFields(path)
  out[name]=dict(fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain,default=f.defaultValue) for f in fs],rows=[list(r) for r in arcpy.da.SearchCursor(path,[f.name for f in fs if f.type!='Geometry'])],indexes=sorted(tuple(f.name for f in i.fields) for i in arcpy.ListIndexes(path)))
 return out
def same(a,b):
 if isinstance(a,(int,float)) and isinstance(b,(int,float)):return math.isclose(a,b,rel_tol=1e-10,abs_tol=1e-10)
 return a==b
results={}
for name in config['names']:
 try:
  expected=snapshot(root+'/'+name+'.gdb');actual=snapshot(P(config['folder']+'/'+name+'.gdb'))
  assert sorted(expected)==sorted(actual),('catalog',list(expected),list(actual))
  for table,want in expected.items():
   got=actual[table]
   assert got['fields']==want['fields'],(table,'fields',got['fields'],want['fields'])
   assert got['indexes']==want['indexes'],(table,'indexes',got['indexes'],want['indexes'])
   assert len(got['rows'])==len(want['rows']),(table,'count')
   for a,b in zip(got['rows'],want['rows']):assert len(a)==len(b) and all(same(x,y) for x,y in zip(a,b)),(table,'row',a,b)
  results[name]=dict(status='matched',tables=len(actual))
 except Exception as e:results[name]=dict(status='failed',error=str(e))
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/feature_class_to_geodatabase/native_reader_results.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print(json.dumps(results,indent=2))
