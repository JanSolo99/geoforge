"""ArcGIS bridge: typed splitting fixtures and folder naming/schema controls."""
import json
import os
arcpy.env.overwriteOutput=True
root=P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
sr=arcpy.SpatialReference(32633)
for name,spatial,rows in [
 ('split_attributes',True,[('West',2),('East',1),('West',1),('East',2),('East',1)]),
 ('split_attribute_table',False,[('West',2),('East',1),('West',1),('East',2),('East',1)]),
 ('split_attribute_names',True,[('A B',1),('7west',2),('a-b',3),('a_b',4),('../path',5),('',6)]),
]:
 suffix='.shp' if spatial else '.dbf'; path=root+'/'+name+suffix
 if spatial:arcpy.management.CreateFeatureclass(root,name+suffix,'POINT',spatial_reference=sr)
 else:arcpy.management.CreateTable(root,name+suffix)
 arcpy.management.AddField(path,'REGION','TEXT',field_length=32)
 arcpy.management.AddField(path,'CLASS','LONG')
 fields=['REGION','CLASS']+(['SHAPE@XY'] if spatial else [])
 with arcpy.da.InsertCursor(path,fields) as c:
  for i,(region,cls) in enumerate(rows):c.insertRow((region,cls)+(((500000+i*10,5000000+i),) if spatial else ()))
results={}
for name,source,fields in [
 ('text','split_attributes.shp','REGION'),('numeric','split_attributes.shp','CLASS'),
 ('multi','split_attributes.shp','REGION;CLASS'),('names','split_attribute_names.shp','REGION'),
 ('table','split_attribute_table.dbf','REGION;CLASS'),('oid','split_attributes.shp','FID'),
]:
 folder=P('/tmp/unigis-split-attributes-'+name)
 os.makedirs(folder,exist_ok=True)
 try:
  arcpy.analysis.SplitByAttributes(root+'/'+source,folder,fields)
  outputs={}
  for filename in sorted(os.listdir(folder)):
   path=folder+'/'+filename
   if not (filename.endswith('.shp') or filename.endswith('.dbf') and not os.path.exists(path[:-4]+'.shp')):continue
   names=[f.name for f in arcpy.ListFields(path) if f.type not in ['Geometry','OID']]
   outputs[filename]={'fields':[(f.name,f.type,f.length,f.precision,f.scale) for f in arcpy.ListFields(path)],'rows':list(arcpy.da.SearchCursor(path,names+(['SHAPE@WKT'] if filename.endswith('.shp') else [])))}
  results[name]={'outputs':outputs,'messages':arcpy.GetMessages()}
 except Exception as e:results[name]={'error':str(e),'messages':arcpy.GetMessages()}
with open(P('/tmp/unigis-split-attributes-probe.json'),'w',newline='\n') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-split-attributes-probe.json')
