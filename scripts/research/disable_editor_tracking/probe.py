"""Public ArcGIS editor-tracking controls; untouched source retained for references."""
import os, json, datetime
root=P('/tmp/unigis-disable-editor-tracking-probe');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
def snap(fc):
 d=arcpy.Describe(fc)
 props=['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']
 fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 return dict(tracking={p:getattr(d,p,None) for p in props},fields=[dict(name=f.name,alias=f.aliasName,type=f.type,required=f.required) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))))
for name in ['all_off','creator_off','created_off','editor_off','edited_off','keep_all','keep_dates','table_all','plain','local_time']:
 if name=='table_all':fc=arcpy.management.CreateTable(gdb,name).getOutput(0)
 else:fc=arcpy.management.CreateFeatureclass(gdb,name,'POINT',spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG')
 if name!='plain':arcpy.management.EnableEditorTracking(fc,'creator','created','editor','edited','ADD_FIELDS','DATABASE_TIME' if name=='local_time' else 'UTC')
 fields=['KEY']+([] if name=='table_all' else ['SHAPE@XY'])
 values=[7]+([] if name=='table_all' else [(500000,5000000)])
 with arcpy.da.InsertCursor(fc,fields) as c:c.insertRow(values)
 if name!='plain':
  with arcpy.da.UpdateCursor(fc,['creator','created','editor','edited']) as c:
   for row in c:c.updateRow(['original',datetime.datetime(2020,1,2,3,4,5),'last',datetime.datetime(2021,2,3,4,5,6)])
shape=arcpy.management.CreateFeatureclass(root,'point.shp','POINT',spatial_reference=sr).getOutput(0)
# Byte-level source states observed later through file-format readers only.
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/editor_tracking_base.gdb'))
flags={
 'creator_off':['DISABLE_CREATOR','NO_DISABLE_CREATION_DATE','NO_DISABLE_LAST_EDITOR','NO_DISABLE_LAST_EDIT_DATE'],
 'created_off':['NO_DISABLE_CREATOR','DISABLE_CREATION_DATE','NO_DISABLE_LAST_EDITOR','NO_DISABLE_LAST_EDIT_DATE'],
 'editor_off':['NO_DISABLE_CREATOR','NO_DISABLE_CREATION_DATE','DISABLE_LAST_EDITOR','NO_DISABLE_LAST_EDIT_DATE'],
 'edited_off':['NO_DISABLE_CREATOR','NO_DISABLE_CREATION_DATE','NO_DISABLE_LAST_EDITOR','DISABLE_LAST_EDIT_DATE'],
 'keep_all':['NO_DISABLE_CREATOR','NO_DISABLE_CREATION_DATE','NO_DISABLE_LAST_EDITOR','NO_DISABLE_LAST_EDIT_DATE'],
 'keep_dates':['DISABLE_CREATOR','NO_DISABLE_CREATION_DATE','DISABLE_LAST_EDITOR','NO_DISABLE_LAST_EDIT_DATE']}
result={}
for name in ['all_off','creator_off','created_off','editor_off','edited_off','keep_all','keep_dates','table_all','plain','local_time','shape']:
 fc=shape if name=='shape' else os.path.join(gdb,name)
 before=snap(fc)
 try:
  arcpy.management.DisableEditorTracking(fc,*flags.get(name,[]))
  messages=arcpy.GetMessages();after=snap(fc)
  arcpy.management.DisableEditorTracking(fc,*flags.get(name,[]))
  result[name]=dict(before=before,after=after,repeat=snap(fc),messages=messages)
 except Exception as e:result[name]=dict(before=before,error=str(e),messages=arcpy.GetMessages(),after=snap(fc))
with open(P('/tmp/unigis-disable-editor-tracking-probe.json'),'w') as f:json.dump(result,f,indent=2,default=str)
print('Saved /tmp/unigis-disable-editor-tracking-probe.json')
