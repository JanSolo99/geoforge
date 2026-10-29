"""Additional public schema/default and name controls; no source fixture edits."""
import os,json
root=P('/mnt/media/ArcGIS_Linux')
base=P('/tmp/unigis-fctg-options')
os.makedirs(base,exist_ok=True)
source=root+'/testdata/geoproc'
named=base+'/9 bad-name.shp'
arcpy.management.CopyFeatures(source+'/wells.shp',named)
results={}
for name,inputs in [
 ('properties',[source+'/extract_base.gdb/sites']),
 ('polygon_gdb',[P('/tmp/unigis-fctg-probe/single.gdb/parcels')]),
 ('name',[named]),
]:
 gdb=arcpy.management.CreateFileGDB(base,name+'.gdb').getOutput(0)
 try:
  arcpy.conversion.FeatureClassToGeodatabase(inputs,gdb)
  arcpy.env.workspace=gdb
  tables={}
  for item in arcpy.ListFeatureClasses():
   p=gdb+'/'+item;fs=arcpy.ListFields(p)
   tables[item]=dict(fields=[dict(name=f.name,alias=f.aliasName,type=f.type,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain,default=f.defaultValue) for f in fs],rows=[list(row) for row in arcpy.da.SearchCursor(p,[f.name for f in fs if f.type!='Geometry'])],indexes=[dict(name=i.name,fields=[f.name for f in i.fields]) for i in arcpy.ListIndexes(p)])
  results[name]=dict(status='success',tables=tables,domains=[d.name for d in arcpy.da.ListDomains(gdb)])
 except Exception as e:results[name]=dict(status='error',messages=str(e)+'\n'+arcpy.GetMessages())
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/feature_class_to_geodatabase/options_results.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print('Saved',len(results),'additional controls')
