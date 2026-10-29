"""Public ArcGIS controls; save the immutable source once before extraction."""
import os, json
root=P('/tmp/unigis-extract-probe')
os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
points=arcpy.management.CreateFeatureclass(gdb,'sites','POINT',spatial_reference=sr).getOutput(0)
for name,typ in [('KEY','LONG'),('STATUS','SHORT'),('LABEL','TEXT')]:
 arcpy.management.AddField(points,name,typ,field_length=30,field_alias='Site '+name)
arcpy.management.CreateDomain(gdb,'StatusDomain','Site status','SHORT','CODED')
arcpy.management.AddCodedValueToDomain(gdb,'StatusDomain',1,'Active')
arcpy.management.AddCodedValueToDomain(gdb,'StatusDomain',2,'Closed')
arcpy.management.AssignDomainToField(points,'STATUS','StatusDomain')
arcpy.management.AssignDefaultToField(points,'STATUS',1)
with arcpy.da.InsertCursor(points,['SHAPE@XY','KEY','STATUS','LABEL']) as c:
 for i in range(1,5):c.insertRow([(500000+i*10,5000000),i,1 if i%2 else 2,'Site '+str(i) if i!=3 else None])
with arcpy.da.UpdateCursor(points,['KEY']) as c:
 for row in c:
  if row[0]==2:c.deleteRow()
arcpy.management.AddIndex(points,'KEY','sites_key')
table=arcpy.management.CreateTable(gdb,'observations').getOutput(0)
arcpy.management.AddField(table,'SITE_KEY','LONG')
arcpy.management.AddField(table,'NOTE','TEXT',field_length=40,field_alias='Observation')
with arcpy.da.InsertCursor(table,['SITE_KEY','NOTE']) as c:
 for row in [(1,'a'),(1,'b'),(3,'c'),(9,'unmatched')]:c.insertRow(row)
arcpy.management.CreateRelationshipClass(points,table,os.path.join(gdb,'site_observations'),'SIMPLE','Observations','Site','NONE','ONE_TO_MANY','NONE','KEY','SITE_KEY')
other=arcpy.management.CreateTable(gdb,'unrelated').getOutput(0)
arcpy.management.AddField(other,'VALUE','LONG')
with arcpy.da.InsertCursor(other,['VALUE']) as c:c.insertRow([77])
plain=arcpy.management.CreateFeatureclass(gdb,'plain','POINT',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(plain,'KEY','LONG')
with arcpy.da.InsertCursor(plain,['SHAPE@XY','KEY']) as c:
 for i in range(3):c.insertRow([(500000+i*10,5000000),i])
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/extract_base.gdb'))

def snapshot(path):
 arcpy.env.workspace=path
 names=sorted(arcpy.ListFeatureClasses()+arcpy.ListTables())
 result={}
 for name in names:
  fc=os.path.join(path,name); d=arcpy.Describe(fc)
  fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
  result[name]=dict(fields=[dict(name=f.name,type=f.type,alias=f.aliasName,nullable=f.isNullable,required=f.required,domain=f.domain,default=f.defaultValue) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))),indexes=[dict(name=x.name,fields=[f.name for f in x.fields],unique=x.isUnique,ascending=x.isAscending) for x in arcpy.ListIndexes(fc)])
 return dict(tables=result,relationships=sorted(d.name for d in arcpy.Describe(path).children if d.dataType=='RelationshipClass'),domains=sorted(d.name for d in arcpy.da.ListDomains(path)))
results={}
controls=[('plain',['plain'],{}),('table_default',['observations'],{}),('table_all',['observations'],dict(all_records_for_tables='ALL_RECORDS_FOR_TABLES')),('related',['sites'],{}),('schema',['sites'],dict(extract_type='SCHEMA_ONLY')),('no_expand',['sites'],dict(expand_feature_classes_and_tables='DO_NOT_ADD',get_related_data='DO_NOT_GET_RELATED')),('all_expanded',['sites'],dict(expand_feature_classes_and_tables='ALL_ROWS')),('both',['sites','observations'],dict(all_records_for_tables='ALL_RECORDS_FOR_TABLES')),('existing',['plain'],{}),('duplicate',['plain','plain'],{})]
for name,items,kwargs in controls:
 args=dict(in_data=[os.path.join(gdb,n) for n in items],out_type='NEW_FILE_GEODATABASE',out_folder_path=root,out_name=name)
 args.update(kwargs)
 if name=='existing':args=dict(in_data=args['in_data'],out_geodatabase=arcpy.management.CreateFileGDB(root,name+'.gdb').getOutput(0))
 try:
  r=arcpy.management.ExtractDataFromGeodatabase(**args)
  dest=os.path.join(root,name+'.gdb');results[name]=dict(status='succeeded',messages=arcpy.GetMessages(),snapshot=snapshot(dest),outputs=[r.getOutput(i) for i in range(r.outputCount)])
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-extract-probe.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print(json.dumps({k:dict(status=v['status'],tables={n:len(t['rows']) for n,t in v.get('snapshot',{}).get('tables',{}).items()},relationships=v.get('snapshot',{}).get('relationships'),error=v.get('error')) for k,v in results.items()},indent=2))
