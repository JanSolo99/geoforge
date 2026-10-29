"""Public EnableCOGO behavior controls, saving untouched source once."""
import os,json
root=P('/tmp/unigis-enable-cogo-probe');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
names=['plain','empty','partial','existing','wrong_string','wrong_integer','point','polygon','table_only','enabled']
for name in names:
 if name=='table_only':fc=arcpy.management.CreateTable(gdb,name).getOutput(0)
 else:fc=arcpy.management.CreateFeatureclass(gdb,name,{'point':'POINT','polygon':'POLYGON'}.get(name,'POLYLINE'),spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 if name in ['partial','existing']:
  arcpy.management.AddField(fc,'Direction','DOUBLE',field_alias='My direction')
  if name=='existing':
   for field in ['Distance','Radius','ArcLength','Radius2']:arcpy.management.AddField(fc,field,'DOUBLE',field_alias='My '+field)
 if name in ['wrong_string','wrong_integer']:
  arcpy.management.AddField(fc,'Direction','TEXT' if name=='wrong_string' else 'LONG')
 if name=='enabled':arcpy.management.EnableCOGO(fc)
 if name not in ['empty','point','polygon','table_only']:
  geom=arcpy.Polyline(arcpy.Array([arcpy.Point(500000,5000000),arcpy.Point(500010,5000000)]),sr)
  with arcpy.da.InsertCursor(fc,['KEY','SHAPE@']) as c:
   c.insertRow([7,geom]);c.insertRow([8,geom]);c.insertRow([9,geom])
  with arcpy.da.UpdateCursor(fc,['KEY']) as c:
   for row in c:
    if row[0]==8:c.deleteRow()
 if name in ['partial','existing','enabled']:
  with arcpy.da.UpdateCursor(fc,['Direction']) as c:
   for row in c:c.updateRow([90])
shape=arcpy.management.CreateFeatureclass(root,'line.shp','POLYLINE',spatial_reference=sr).getOutput(0)
with arcpy.da.InsertCursor(shape,['SHAPE@']) as c:c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Point(0,0),arcpy.Point(10,0)]),sr)])
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/enable_cogo_base.gdb'))
def snap(fc):
 d=arcpy.Describe(fc);fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 return dict(cogo=getattr(d,'isCOGOEnabled',None),fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))))
results={}
for name in names+['shape']:
 fc=shape if name=='shape' else os.path.join(gdb,name);before=snap(fc)
 try:
  arcpy.management.EnableCOGO(fc);messages=arcpy.GetMessages();after=snap(fc)
  arcpy.management.EnableCOGO(fc)
  results[name]=dict(before=before,after=after,repeat=snap(fc),messages=messages,repeat_messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(before=before,error=str(e),messages=arcpy.GetMessages(),after=snap(fc))
with open(P('/tmp/unigis-enable-cogo-probe.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-enable-cogo-probe.json')
