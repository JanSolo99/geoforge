"""Independent public arcpy read of disposable native EnableCOGO output."""
import os,json
base=P('/tmp/unigis-enable-cogo-native.gdb')
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/enable_cogo/probe_results.json')) as f:oracle=json.load(f)
results={}
for name in ['plain','empty','partial','existing','enabled']:
 fc=os.path.join(base,name);d=arcpy.Describe(fc)
 fields=[f.name for f in arcpy.ListFields(fc) if f.type!='Geometry']
 actual=dict(cogo=getattr(d,'isCOGOEnabled',None),fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain) for f in arcpy.ListFields(fc)],rows=list(arcpy.da.SearchCursor(fc,fields+['SHAPE@JSON'])))
 actual=json.loads(json.dumps(actual))
 assert actual==oracle[name]['after'],(name,actual,oracle[name]['after'])
 results[name]=actual
with open(P('/tmp/unigis-enable-cogo-native.json'),'w') as f:json.dump(results,f,indent=2)
print('Native public ArcGIS schema/COGO/rows/geometry matches five controls PASS')
