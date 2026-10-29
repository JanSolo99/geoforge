"""Read saved native outputs with public arcpy and compare to ArcGIS outputs."""
import os,json
native=P('/tmp/unigis-extract-native');reference=P('/mnt/media/ArcGIS_Linux/testdata/arcgis-reference')
def snap(gdb):
 arcpy.env.workspace=gdb
 classes={}
 for name in sorted(arcpy.ListFeatureClasses()+arcpy.ListTables()):
  table=os.path.join(gdb,name);d=arcpy.Describe(table)
  attrs=[f.name for f in arcpy.ListFields(table) if f.type!='Geometry']
  fields=[dict(name=f.name,type=f.type,alias=f.aliasName,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain,default=f.defaultValue,precision=f.precision) for f in arcpy.ListFields(table)]
  # Index names contain destination DSIDs; compare the indexed fields instead.
  indexes=sorted(sorted(f.name.lower() for f in i.fields) for i in arcpy.ListIndexes(table))
  classes[name]=dict(fields=fields,rows=list(arcpy.da.SearchCursor(table,attrs+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))),indexes=indexes)
 domains=[dict(name=x.name,description=x.description,type=x.type,kind=x.domainType,split=x.splitPolicy,merge=x.mergePolicy,values=x.codedValues if x.domainType=='CodedValue' else list(x.range)) for x in arcpy.da.ListDomains(gdb)]
 relationships={}
 for child in arcpy.Describe(gdb).children:
  if child.dataType=='RelationshipClass':
   d=arcpy.Describe(child.catalogPath)
   relationships[child.name]={p:getattr(d,p) for p in ['cardinality','isComposite','isAttributed','originClassNames','destinationClassNames','originClassKeys','destinationClassKeys','forwardPathLabel','backwardPathLabel']}
 return dict(classes=classes,domains=sorted(domains,key=lambda d:d['name']),relationships=relationships)
results={}
for name in sorted(os.listdir(native)):
 if not name.endswith('.gdb'):continue
 a=snap(os.path.join(reference,name));b=snap(os.path.join(native,name))
 results[name]=dict(matches=a==b,expected=a,actual=b)
 print(name,'PASS' if a==b else 'FAIL')
 if a!=b:
  for group in a:
   if a[group]!=b[group]:print(' different',group)
with open(P('/tmp/unigis-extract-native.json'),'w') as f:json.dump(results,f,indent=2,default=str)
assert all(v['matches'] for v in results.values()),'native reader differs'
