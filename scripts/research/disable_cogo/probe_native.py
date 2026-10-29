"""Read a disposable native result through public ArcGIS APIs."""
import json
fc=P('/tmp/unigis-disable-cogo-native.gdb/enabled')
r=dict(enabled=arcpy.Describe(fc).isCOGOEnabled,fields=[dict(name=f.name,type=f.type,alias=f.aliasName,required=f.required) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,['OID@','KEY','Direction','Distance','Radius','ArcLength','Radius2','SHAPE@JSON'])))
assert r['enabled'] is False
assert all(not f['required'] for f in r['fields'] if f['name'] in ['Direction','Distance','Radius','ArcLength','Radius2'])
assert r['rows'][0][1:4]==(7,90.0,10.0)
with open(P('/tmp/unigis-disable-cogo-native.json'),'w') as f:json.dump(r,f,indent=2)
print('Native COGO disabled and dimension values retained:',r['enabled'],len(r['fields']),len(r['rows']))
