"""Public arcpy controls; only disposable memory datasets are modified."""
import json
arcpy.env.overwriteOutput=True
arcpy.env.parallelProcessingFactor='0'
sr=arcpy.SpatialReference(32633)
cases={
 'two_points':('POINT',[[[0,0]],[[0.6,0]]]),
 'three_points':('POINT',[[[0,0]],[[0.6,0]],[[1.2,0]]]),
 'triangle':('POINT',[[[0,0]],[[0.8,0]],[[0,0.8]]]),
 'cross':('POLYLINE',[[[0,0],[10,10]],[[0,10],[10,0]]]),
 'near_edge':('POLYLINE',[[[0,0],[10,0]],[[5,0.6],[5,10]]]),
 'near_parallel':('POLYLINE',[[[0,0],[10,0]],[[0,0.6],[10,0.6]]]),
 'self_cross':('POLYLINE',[[[0,0],[10,10],[0,10],[10,0]]]),
 'short':('POLYLINE',[[[0,0],[0.6,0]],[[10,0],[20,0]]]),
}
results={}
for name,(kind,rows) in cases.items():
 for reverse in [False,True]:
  layer='memory/pi_control'
  arcpy.management.CreateFeatureclass('memory','pi_control',kind,spatial_reference=sr)
  arcpy.management.AddField(layer,'KEY','LONG')
  with arcpy.da.InsertCursor(layer,['KEY','SHAPE@']) as c:
   for i,coords in (list(enumerate(rows))[::-1] if reverse else enumerate(rows)):
    pts=[arcpy.Point(500000+x,5000000+y) for x,y in coords]
    geom=arcpy.PointGeometry(pts[0],sr) if kind=='POINT' else arcpy.Polyline(arcpy.Array(pts),sr)
    c.insertRow((i,geom))
  before=list(arcpy.da.SearchCursor(layer,['KEY','SHAPE@JSON']))
  try:
   arcpy.analysis.PairwiseIntegrate([layer],'1 Meters')
   results[name+('_reverse' if reverse else '')]={'before':before,'after':list(arcpy.da.SearchCursor(layer,['KEY','SHAPE@JSON'])),'messages':arcpy.GetMessages()}
  except Exception as e:results[name+str(reverse)]={'error':str(e)}
  arcpy.management.Delete(layer)
with open(P('/tmp/unigis-pi-probe.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-pi-probe.json')
