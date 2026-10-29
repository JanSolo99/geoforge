"""Public DisableCOGO behavior, fields and geodatabase definition controls."""
import os,json
root=P('/tmp/unigis-disable-cogo-probe');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
def snapshot(fc):
 d=arcpy.Describe(fc)
 fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,required=f.required,nullable=f.isNullable,domain=f.domain) for f in arcpy.ListFields(fc)]
 attrs=[f.name for f in arcpy.ListFields(fc) if f.type not in ['Geometry','OID']]
 definition=None
 if '.gdb' in fc:
  with arcpy.da.SearchCursor(os.path.join(gdb,'GDB_Items'),['Name','Definition']) as c:
   for name,xml in c:
    if name==os.path.basename(fc):definition=xml
 return dict(cogo=getattr(d,'isCOGOEnabled',None),fields=fields,rows=list(arcpy.da.SearchCursor(fc,['OID@']+attrs+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))),definition=definition)
for name,kind in [('enabled','POLYLINE'),('empty_enabled','POLYLINE'),('plain','POLYLINE'),('custom','POLYLINE'),('point','POINT'),('polygon','POLYGON')]:
 fc=arcpy.management.CreateFeatureclass(gdb,name,kind,spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 if kind=='POLYLINE' and name!='empty_enabled':
  geom=arcpy.Polyline(arcpy.Array([arcpy.Point(500000,5000000),arcpy.Point(500010,5000000)]),sr)
  with arcpy.da.InsertCursor(fc,['KEY','SHAPE@']) as c:c.insertRow((7,geom))
 if name in ['enabled','empty_enabled']:arcpy.management.EnableCOGO(fc)
 if name=='enabled':
  with arcpy.da.UpdateCursor(fc,['Direction','Distance']) as c:
   for row in c:row[:]=[90,10];c.updateRow(row)
 if name=='custom':arcpy.management.AddField(fc,'Direction','DOUBLE',field_alias='My direction')
arcpy.management.CreateTable(gdb,'table_only')
shape=arcpy.management.CreateFeatureclass(root,'plain.shp','POLYLINE',spatial_reference=sr).getOutput(0)
# Preserve the untouched source state once for reference generation.
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/cogo_base.gdb'))
results={}
for name in ['enabled','empty_enabled','plain','custom','point','polygon','table_only','shape']:
 fc=shape if name=='shape' else os.path.join(gdb,name)
 before=snapshot(fc)
 try:
  arcpy.management.DisableCOGO(fc)
  messages=arcpy.GetMessages();after=snapshot(fc)
  arcpy.management.DisableCOGO(fc)
  results[name]=dict(before=before,after=after,repeat=snapshot(fc),messages=messages,repeat_messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(before=before,error=str(e),messages=arcpy.GetMessages(),after=snapshot(fc))
with open(P('/tmp/unigis-disable-cogo-probe.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-disable-cogo-probe.json')
