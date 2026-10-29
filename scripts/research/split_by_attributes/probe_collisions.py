"""ArcGIS bridge: filename allocation against input order and existing outputs."""
import json
import os
arcpy.env.overwriteOutput=True
sr=arcpy.SpatialReference(32633)
results={}
for label,values in [('reverse',['a_b','A B']),('case',['a','A']),('punctuation',['a.b','a-b','_abc','9abc']),('existing',['East'])]:
 arcpy.management.CreateFeatureclass('memory','sba_names','POINT',spatial_reference=sr)
 arcpy.management.AddField('memory/sba_names','REGION','TEXT',field_length=32)
 with arcpy.da.InsertCursor('memory/sba_names',['SHAPE@XY','REGION']) as c:
  for i,v in enumerate(values):c.insertRow(((500000+i,5000000),v))
 folder=P('/tmp/unigis-sba-collision-'+label);os.makedirs(folder,exist_ok=True)
 if label=='existing':arcpy.management.CopyFeatures('memory/sba_names',folder+'/East.shp')
 arcpy.analysis.SplitByAttributes('memory/sba_names',folder,'REGION')
 outputs={}
 for name in sorted(os.listdir(folder)):
  if name.endswith('.shp'):outputs[name]=list(arcpy.da.SearchCursor(folder+'/'+name,['REGION']))
 results[label]=outputs
 arcpy.management.Delete('memory/sba_names')
with open(P('/tmp/unigis-sba-collisions.json'),'w',newline='\n') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-sba-collisions.json')
