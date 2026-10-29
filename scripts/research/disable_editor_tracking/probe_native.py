"""Independent public ArcGIS read of disposable native outputs (no edits)."""
import os,json
base=P('/tmp/unigis-dtrack-native-fixed')
results={}
for workspace,names in [('editor_tracking_base.gdb',['all_off','creator_off','table_all','local_time']),('editor_tracking_extra.gdb',['grouped/tracked','grouped/plain','custom'])]:
 for name in names:
  fc=os.path.join(base,workspace,*name.split('/'));d=arcpy.Describe(fc)
  fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
  if hasattr(d,'shapeType'):fields+=['SHAPE@JSON']
  results[name]=dict(tracking={p:getattr(d,p,None) for p in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']},fields=[dict(name=f.name,alias=f.aliasName,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields)))
assert results['all_off']['tracking']['editorTrackingEnabled'] is False
assert results['table_all']['tracking']['editorTrackingEnabled'] is False
assert results['creator_off']['tracking']['editorTrackingEnabled'] is True
assert results['creator_off']['tracking']['creatorFieldName']==''
assert results['creator_off']['tracking']['editorFieldName']=='editor'
assert results['local_time']['tracking']['isTimeInUTC'] is False
assert results['grouped/tracked']['tracking']['editorTrackingEnabled'] is False
assert results['custom']['fields'][2]['alias']=='Original author'
assert results['custom']['fields'][2]['domain']=='authors'
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/disable_editor_tracking/probe_results.json')) as f:oracle=json.load(f)
normalized=json.loads(json.dumps(results,default=str))
for name in ['all_off','creator_off','table_all','local_time']:
 assert normalized[name]['tracking']==oracle[name]['after']['tracking'], name
 assert normalized[name]['rows']==oracle[name]['before']['rows'], name
with open(P('/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/manifest.json')) as f:references=json.load(f)['cases']
for name in ['all_off','creator_off','table_all','local_time']:
 expected=references['dtrack_'+name]['editor_tracking']['editable']
 assert all(field['editable']==expected[field['name']] for field in normalized[name]['fields']), name
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/disable_editor_tracking/extra_results.json')) as f:extra=json.load(f)
for name,expected in [('grouped/tracked',extra['dataset']['after'][0]),('grouped/plain',extra['dataset']['after'][1]),('custom',extra['custom']['after'][0])]:
 assert normalized[name]['tracking']==expected['tracking'], name
 assert normalized[name]['fields']==expected['fields'], name
with open(P('/tmp/unigis-dtrack-native-fixed.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print('Native output read and assertions PASS; saved /tmp/unigis-dtrack-native-fixed.json')
