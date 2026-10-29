"""Independent public ArcGIS read of native tracking configuration."""
import os,json
base=P('/tmp/unigis-enable-tracking-native.gdb')
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/enable_editor_tracking/probe_results.json')) as f:oracle=json.load(f)
results={}
for name,control in oracle.items():
 if 'error' in control:continue
 fc=os.path.join(base,name);d=arcpy.Describe(fc);fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 actual=dict(tracking={p:getattr(d,p,None) for p in ['editorTrackingEnabled','creatorFieldName','createdAtFieldName','editorFieldName','editedAtFieldName','isTimeInUTC']},fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+(['SHAPE@JSON'] if hasattr(d,'shapeType') else []))))
 actual=json.loads(json.dumps(actual,default=str));assert actual==control['after'],(name,actual,control['after']);results[name]=actual
with open(P('/tmp/unigis-enable-tracking-native.json'),'w') as f:json.dump(results,f,indent=2)
print('Native public ArcGIS metadata/fields/values/geometry match ten controls PASS')
