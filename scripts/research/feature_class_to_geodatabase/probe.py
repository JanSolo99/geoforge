"""Public FeatureClassToGeodatabase controls; all writes to fresh /tmp workspaces."""
import os,json
root=P('/mnt/media/ArcGIS_Linux')
folder=P('/tmp/unigis-fctg-probe')
os.makedirs(folder,exist_ok=True)
source=root+'/testdata/geoproc'
results={}
for name,inputs,seed,dataset in [
 ('single',[source+'/parcels.shp'],[],None),
 ('batch',[source+'/parcels.shp',source+'/roads.shp',source+'/wells.shp'],[],None),
 ('repeat',[source+'/parcels.shp']*2,[],None),
 ('collision',[source+'/parcels.shp'],['parcels','parcels_1'],None),
 ('gdb',[source+'/globalids_base.gdb/points'],[],None),
 ('empty',[source+'/globalids_base.gdb/empty'],[],None),
 ('dataset',[source+'/parcels.shp'],[],'group'),
 ('table',[source+'/globalids_base.gdb/records'],[],None),
 ('bad_batch',[source+'/parcels.shp',source+'/missing.shp'],[],None),
 ('missing_output',[source+'/parcels.shp'],[],False),
]:
 gdb=folder+'/'+name+'.gdb'
 if dataset is not False:
  arcpy.management.CreateFileGDB(folder,name+'.gdb')
  for item in seed:arcpy.management.CopyFeatures(source+'/parcels.shp',gdb+'/'+item)
 if dataset:
  arcpy.management.CreateFeatureDataset(gdb,dataset,arcpy.Describe(source+'/parcels.shp').spatialReference)
 output=gdb+'/'+dataset if dataset else gdb
 try:
  r=arcpy.conversion.FeatureClassToGeodatabase(inputs,output)
  status='success'; messages=arcpy.GetMessages()
 except Exception as e:status='error';messages=str(e)+'\n'+arcpy.GetMessages()
 arcpy.env.workspace=output
 items=sorted(arcpy.ListFeatureClasses() or []) if arcpy.Exists(output) else []
 tables={}
 for item in items:
  path=output+'/'+item
  fs=arcpy.ListFields(path)
  attrs=[f.name for f in fs if f.type not in ('OID','Geometry')]
  tables[item]=dict(fields=[dict(name=f.name,alias=f.aliasName,type=f.type,length=f.length,precision=f.precision,scale=f.scale,nullable=f.isNullable,required=f.required,editable=f.editable,domain=f.domain,default=f.defaultValue) for f in fs],
                    rows=[list(row) for row in arcpy.da.SearchCursor(path,['OID@',*attrs])],
                    shape=arcpy.Describe(path).shapeType,has_z=arcpy.Describe(path).hasZ,has_m=arcpy.Describe(path).hasM)
 results[name]=dict(status=status,messages=messages,tables=tables)
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/feature_class_to_geodatabase/results.json'),'w') as f:json.dump(results,f,indent=2,default=str)
print('Saved',len(results),'FeatureClassToGeodatabase controls')
