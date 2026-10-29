"""Probe GPS schema/domain behavior on disposable geodatabases."""
import json,os
root=P('/tmp/unigis-gps-oracle')
os.makedirs(root,exist_ok=True)
arcpy.env.overwriteOutput=True
sr=arcpy.SpatialReference(32633)
def fields(fc):return [{'name':f.name,'alias':f.aliasName,'type':f.type,'length':f.length,'nullable':f.isNullable,'required':f.required,'domain':f.domain,'default':f.defaultValue} for f in arcpy.ListFields(fc)]
def domains(gdb):
 out=[]
 for d in arcpy.da.ListDomains(gdb):
  out.append({'name':d.name,'description':d.description,'type':d.type,'kind':d.domainType,'split':d.splitPolicy,'merge':d.mergePolicy,'values':d.codedValues if d.domainType=='CodedValue' else list(d.range)})
 return out
results={}
for kind in ['POINT','POLYLINE','POLYGON','MULTIPOINT']:
 for offset in [False,True]:
  name=kind.lower()+('_offset' if offset else '')
  gdb=arcpy.management.CreateFileGDB(root,name+'.gdb').getOutput(0)
  fc=arcpy.management.CreateFeatureclass(gdb,'features',kind,spatial_reference=sr).getOutput(0)
  try:
   arcpy.management.AddGPSMetadataFields(fc,'INCLUDE_OFFSET_FIELDS' if offset else 'EXCLUDE_OFFSET_FIELDS')
   results[name]={'fields':fields(fc),'domains':domains(gdb),'messages':arcpy.GetMessages()}
   arcpy.management.AddGPSMetadataFields(fc)
   results[name]['repeat']={'fields':fields(fc),'messages':arcpy.GetMessages()}
  except Exception as e:results[name]={'error':str(e),'messages':arcpy.GetMessages()}
  arcpy.management.Delete(gdb)
# File formats known unsupported for full-text indexes, no source fixtures changed.
gdb=arcpy.management.CreateFileGDB(root,'fulltext.gdb').getOutput(0)
fc=arcpy.management.CreateFeatureclass(gdb,'features','POINT',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(fc,'TEXT','TEXT')
try:arcpy.management.AddFullTextIndex(fc,'TEXT');results['fulltext']={'unexpected_success':True}
except Exception as e:results['fulltext']={'error':str(e),'messages':arcpy.GetMessages()}
arcpy.management.Delete(gdb)
with open(P('/tmp/unigis-gps-probe.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-gps-probe.json')
