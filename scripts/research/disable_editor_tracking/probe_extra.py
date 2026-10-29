"""Supplemental controls: feature datasets, custom field metadata, boolean arguments."""
import os,json
root=P('/tmp/unigis-disable-editor-tracking-extra');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'extra.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
fd=arcpy.management.CreateFeatureDataset(gdb,'grouped',sr).getOutput(0)
for name in ['tracked','plain']:
 fc=arcpy.management.CreateFeatureclass(fd,name,'POINT').getOutput(0)
 if name=='tracked':arcpy.management.EnableEditorTracking(fc,'maker','made','changer','changed','ADD_FIELDS','UTC')
fc=arcpy.management.CreateFeatureclass(gdb,'custom','POINT',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(fc,'maker','TEXT',field_length=40,field_alias='Original author')
arcpy.management.AddField(fc,'made','DATE',field_alias='Original creation')
arcpy.management.CreateDomain(gdb,'authors','Author choices','TEXT','CODED')
arcpy.management.AddCodedValueToDomain(gdb,'authors','a','Author A')
arcpy.management.AssignDomainToField(fc,'maker','authors')
arcpy.management.EnableEditorTracking(fc,'maker','made','','','NO_ADD_FIELDS','UTC')
# Save once before running the controls.
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/editor_tracking_extra.gdb'))
def snap(path):
 d=arcpy.Describe(path)
 return dict(tracking={n:getattr(d,n,None) for n in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']},fields=[dict(name=f.name,alias=f.aliasName,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(path)])
results={}
for name,path,args in [('dataset',fd,[]),('custom',fc,[]),('boolean',fc,[True,False,True,False]),('bad_bool',fc,['garbage']),('empty_group',fd,[False,False,False,False])]:
 targets=[os.path.join(fd,n) for n in ['tracked','plain']] if path==fd else [path]
 before=[snap(p) for p in targets]
 try:
  arcpy.management.DisableEditorTracking(path,*args)
  results[name]=dict(before=before,after=[snap(p) for p in targets],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(before=before,error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-disable-editor-tracking-extra.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-disable-editor-tracking-extra.json')
