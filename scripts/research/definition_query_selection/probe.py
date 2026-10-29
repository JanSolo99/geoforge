"""Public controls for the required map selection/definition-query state."""
import json
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')
results={}
layer=arcpy.management.MakeFeatureLayer(source,'definition_selection_probe').getOutput(0)
for name,value,selection in [('file',source,None),('layer_no_selection',layer,None),('layer_selected',layer,'PARCEL_ID IN (1,3,7)')]:
 if selection:arcpy.management.SelectLayerByAttribute(layer,'NEW_SELECTION',selection)
 try:
  r=arcpy.management.GenerateDefinitionQueryFromSelection(value)
  results[name]=dict(status='succeeded',messages=arcpy.GetMessages(),outputs=[str(r.getOutput(i)) for i in range(r.outputCount)])
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-definition-selection.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
