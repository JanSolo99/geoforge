"""Public AddGlobalIDs controls, run through tools/arcgis-bridge/run_code.py."""
import os,json,shutil
root=P('/tmp/unigis-globalids-probe');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'control.gdb').getOutput(0)
sr=arcpy.SpatialReference(3857)
for name in ['points','repeat','collision','empty']:
 fc=arcpy.management.CreateFeatureclass(gdb,name,'POINT',spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 if name!='empty':
  with arcpy.da.InsertCursor(fc,['SHAPE@XY','KEY']) as c:
   for i in range(3):c.insertRow([(i,i),i+1])
 if name=='collision':arcpy.management.AddField(fc,'GlobalID','TEXT')
t=arcpy.management.CreateTable(gdb,'records').getOutput(0)
arcpy.management.AddField(t,'KEY','LONG')
with arcpy.da.InsertCursor(t,['KEY']) as c:c.insertRow([1])
fd=arcpy.management.CreateFeatureDataset(gdb,'group',sr).getOutput(0)
arcpy.management.CreateFeatureclass(fd,'child','POINT',spatial_reference=sr)
# Immutable fixture saved before any controls.
shutil.copytree(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/globalids_base.gdb'),ignore=shutil.ignore_patterns('*.lock'))
results={}
def snap(path):
 d=arcpy.Describe(path)
 fields=[dict(name=f.name,alias=f.aliasName,type=f.type,nullable=f.isNullable,required=f.required,editable=f.editable,length=f.length) for f in arcpy.ListFields(path)]
 names=[f.name for f in arcpy.ListFields(path) if f.type!='Geometry']
 return dict(globalIDFieldName=getattr(d,'globalIDFieldName',None),hasGlobalID=getattr(d,'hasGlobalID',None),fields=fields,rows=list(arcpy.da.SearchCursor(path,names)),indexes=[dict(name=i.name,unique=i.isUnique,ascending=i.isAscending,fields=[f.name for f in i.fields]) for i in arcpy.ListIndexes(path)])
for name,paths in [('points',['points']),('repeat',['repeat']),('repeat_again',['repeat']),('table',['records']),('empty',['empty']),('collision',['collision']),('dataset',['group']),('missing',['missing'])]:
 targets=[os.path.join(gdb,n) for n in paths]
 try:
  arcpy.management.AddGlobalIDs(targets)
  result=dict(status='succeeded',messages=arcpy.GetMessages())
 except Exception as e:result=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
 target=os.path.join(gdb,'group','child') if name=='dataset' else targets[0]
 if arcpy.Exists(target):result['after']=snap(target)
 results[name]=result
with open(P('/tmp/unigis-globalids-probe.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print(json.dumps(results,indent=2,default=str))
