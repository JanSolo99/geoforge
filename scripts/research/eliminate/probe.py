"""Public controls for selected polygons versus unselected merge destinations."""
import os,json
root=P('/tmp/unigis-eliminate-probe');os.makedirs(root,exist_ok=False)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')
layer=arcpy.management.MakeFeatureLayer(source,'eliminate_selection_probe').getOutput(0)
results={}
for name,value,selected in [('file',source,False),('unselected',layer,False),('selected',layer,True)]:
 if selected:arcpy.management.SelectLayerByAttribute(layer,'NEW_SELECTION','PARCEL_ID = 1')
 out=os.path.join(root,name+'.shp')
 try:
  arcpy.management.Eliminate(value,out)
  results[name]=dict(status='succeeded',source_count=int(arcpy.management.GetCount(source).getOutput(0)),output_count=int(arcpy.management.GetCount(out).getOutput(0)),keys=[r[0] for r in arcpy.da.SearchCursor(out,['PARCEL_ID'])],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-eliminate-probe.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
