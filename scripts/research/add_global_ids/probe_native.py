"""ArcGIS reader verifies native UUID schema, index, identity and interoperability."""
import os,json,uuid
root=P('/tmp/unigis-globalids-native/output.gdb')
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/globalids_base.gdb')
with open(P('/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/manifest.json')) as f:cases=json.load(f)['cases']
results={};all_ids=set()
for name in ['points','repeat','records','empty','collision','child']:
 path=os.path.join(root,'group','child') if name=='child' else os.path.join(root,name)
 old=os.path.join(source,'group','child') if name=='child' else os.path.join(source,name)
 case='gids_group' if name=='child' else 'gids_'+name
 expected=cases[case]['targets'][name]
 try:
  d=arcpy.Describe(path)
  fields=[dict(name=f.name,type=f.type,nullable=f.isNullable,required=f.required,editable=f.editable,length=f.length) for f in arcpy.ListFields(path)]
  indexes=[dict(name=i.name,fields=[f.name for f in i.fields]) for i in arcpy.ListIndexes(path)]
  ids=[r[0] for r in arcpy.da.SearchCursor(path,[d.globalIDFieldName])]
  old_fields=[f.name for f in arcpy.ListFields(old) if f.type!='Geometry']
  if hasattr(d,'shapeType'):old_fields+=['SHAPE@JSON']
  before=list(arcpy.da.SearchCursor(old,old_fields));after=list(arcpy.da.SearchCursor(path,old_fields))
  errors=[]
  if fields!=expected['fields']:errors.append('field schema differs')
  if indexes!=expected['indexes']:errors.append('indexes differ')
  if not d.hasGlobalID or d.globalIDFieldName!=expected['globalIDFieldName']:errors.append('GlobalID identity differs')
  if before!=after:errors.append('source attributes/OIDs/geometry changed')
  if any(uuid.UUID(i).version!=4 for i in ids) or any(i in all_ids for i in ids) or len(set(ids))!=len(ids):errors.append('invalid/duplicate UUID4')
  all_ids.update(ids)
  arcpy.management.AddGlobalIDs(path)
  if [r[0] for r in arcpy.da.SearchCursor(path,[d.globalIDFieldName])]!=ids:errors.append('ArcGIS repeat changed native IDs')
  results[name]=dict(matches=not errors,errors=errors,fields=fields,indexes=indexes,ids=ids,preserved_rows=after)
 except Exception as e:results[name]=dict(matches=False,error=str(e))
with open(P('/tmp/unigis-globalids-native-reader.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print(json.dumps(results,indent=2,default=str))
