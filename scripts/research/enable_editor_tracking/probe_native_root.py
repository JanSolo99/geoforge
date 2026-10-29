"""Independent ArcGIS check of a native-created table's new tracking root properties."""
import json
fc=P('/tmp/unigis-enable-tracking-native-root.gdb/native');d=arcpy.Describe(fc)
result=dict(tracking={p:getattr(d,p,None) for p in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']},editable={f.name:f.editable for f in arcpy.ListFields(fc)},rows=int(arcpy.management.GetCount(fc).getOutput(0)))
assert result['tracking']==dict(editorTrackingEnabled=True,creatorFieldName='maker',createdAtFieldName='made',editorFieldName='changer',editedAtFieldName='changed',isTimeInUTC=True),result
assert result['rows']==0 and all(result['editable'][f] is False for f in ['maker','made','changer','changed']),result
with open(P('/tmp/unigis-enable-tracking-native-root.json'),'w') as f:json.dump(result,f,indent=2)
print('Native-created table tracking root properties/read-only fields PASS')
