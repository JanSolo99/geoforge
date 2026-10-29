"""Public EnableEditorTracking controls. Save untouched source once."""
import os,json,datetime
root=P('/tmp/unigis-enable-editor-tracking-probe');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
names=['add_all','table_add','creator_only','dates_only','local_time','existing','missing','wrong','no_roles','active_same','active_extra','active_switch','active_zone','shared']
for name in names:
 fc=(arcpy.management.CreateTable(gdb,name) if name=='table_add' else arcpy.management.CreateFeatureclass(gdb,name,'POINT',spatial_reference=sr)).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 if name in ['existing','active_same','active_extra','active_switch','active_zone','shared']:
  for f,t in [('maker','TEXT'),('made','DATE'),('changer','TEXT'),('changed','DATE')]:arcpy.management.AddField(fc,f,t,field_length=40 if t=='TEXT' else None,field_alias='My '+f)
 if name=='wrong':arcpy.management.AddField(fc,'maker','LONG')
 with arcpy.da.InsertCursor(fc,['KEY']+([] if name=='table_add' else ['SHAPE@XY'])) as c:c.insertRow([7]+([] if name=='table_add' else [(500000,5000000)]))
 if name in ['existing','active_same','active_extra','active_switch','active_zone','shared']:
  with arcpy.da.UpdateCursor(fc,['maker','made','changer','changed']) as c:
   for row in c:c.updateRow(['original',datetime.datetime(2020,1,2,3,4,5),'last',datetime.datetime(2021,2,3,4,5,6)])
 if name.startswith('active_'):arcpy.management.EnableEditorTracking(fc,'maker','made','','','NO_ADD_FIELDS','UTC')
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/enable_tracking_base.gdb'))
args={
 'add_all':['maker','made','changer','changed','ADD_FIELDS'],
 'table_add':['maker','made','changer','changed','ADD_FIELDS'],
 'creator_only':['maker','','','','ADD_FIELDS'],
 'dates_only':['','made','','changed','ADD_FIELDS'],
 'local_time':['maker','made','changer','changed','ADD_FIELDS','DATABASE_TIME'],
 'existing':['maker','made','changer','changed'],
 'missing':['maker','made','changer','changed'],
 'wrong':['maker','','','','ADD_FIELDS'],
 'no_roles':[],
 'active_same':['maker','made','',''],
 'active_extra':['','','changer','changed'],
 'active_switch':['other','made','','','ADD_FIELDS'],
 'active_zone':['maker','made','','','NO_ADD_FIELDS','DATABASE_TIME'],
 'shared':['maker','made','maker','made']}
def snap(fc):
 d=arcpy.Describe(fc);fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 return dict(tracking={p:getattr(d,p,None) for p in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']},fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))))
result={}
for name in names:
 fc=os.path.join(gdb,name);before=snap(fc)
 try:
  arcpy.management.EnableEditorTracking(fc,*args[name]);messages=arcpy.GetMessages();after=snap(fc)
  arcpy.management.EnableEditorTracking(fc,*args[name])
  result[name]=dict(args=args[name],before=before,after=after,repeat=snap(fc),messages=messages)
 except Exception as e:result[name]=dict(args=args[name],before=before,error=str(e),messages=arcpy.GetMessages(),after=snap(fc))
with open(P('/tmp/unigis-enable-editor-tracking-probe.json'),'w') as f:json.dump(result,f,indent=2,default=str)
print('Saved /tmp/unigis-enable-editor-tracking-probe.json')
