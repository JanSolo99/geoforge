"""ArcGIS bridge: independent aligned-overlap/containment boundary controls."""
import json
arcpy.env.overwriteOutput=True
sr=arcpy.SpatialReference(32633)
def rect(x,y,w,h):
 return arcpy.Polygon(arcpy.Array([arcpy.Point(500000+a,5000000+b) for a,b in [(x,y),(x,y+h),(x+w,y+h),(x+w,y),(x,y)]]),sr)
results={}
for name,b in [('aligned',rect(5,0,10,10)),('contained_edge',rect(0,0,5,5)),('contained_middle',rect(2,2,2,2))]:
 arcpy.management.CreateFeatureclass('memory','ppn_probe','POLYGON',spatial_reference=sr)
 with arcpy.da.InsertCursor('memory/ppn_probe',['SHAPE@']) as c:
  c.insertRow((rect(0,0,10,10),));c.insertRow((b,))
 for flag in ['NO_AREA_OVERLAP','AREA_OVERLAP']:
  arcpy.analysis.PairwisePolygonNeighbors('memory/ppn_probe','memory/ppn_out','',flag)
  fields=[f.name for f in arcpy.ListFields('memory/ppn_out') if f.type!='OID']
  results[name+'_'+flag]={'fields':fields,'rows':list(arcpy.da.SearchCursor('memory/ppn_out',fields))}
 arcpy.management.Delete('memory/ppn_probe')
with open(P('/tmp/unigis-ppn-overlap.json'),'w',newline='\n') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-ppn-overlap.json')
