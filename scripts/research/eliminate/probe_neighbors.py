"""Selected sliver must merge into an unselected adjacent polygon."""
import os,json
root=P('/tmp/unigis-eliminate-neighbors');os.makedirs(root,exist_ok=False)
sr=arcpy.SpatialReference(32633)
source=arcpy.management.CreateFeatureclass(root,'source.shp','POLYGON',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(source,'KEY','LONG')
with arcpy.da.InsertCursor(source,['KEY','SHAPE@']) as c:
 for k,x0,x1 in [(1,0,10),(2,10,12),(3,12,32)]:
  c.insertRow([k,arcpy.Polygon(arcpy.Array([arcpy.Point(500000+x,5000000+y) for x,y in [(x0,0),(x0,10),(x1,10),(x1,0),(x0,0)]]),sr)])
layer=arcpy.management.MakeFeatureLayer(source,'eliminate_neighbors_probe').getOutput(0)
arcpy.management.SelectLayerByAttribute(layer,'NEW_SELECTION','KEY = 2')
out=os.path.join(root,'merged.shp');arcpy.management.Eliminate(layer,out,'AREA');messages=arcpy.GetMessages()
result=dict(source_count=3,selected_keys=[2],output=list(arcpy.da.SearchCursor(out,['KEY','SHAPE@AREA'])),messages=messages)
with open(P('/tmp/unigis-eliminate-neighbors.json'),'w') as f:json.dump(result,f,indent=2)
print(json.dumps(result,indent=2))
