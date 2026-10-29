"""ArcGIS bridge: defaults and boundary rules for Remove Overlap (multiple)."""
import json
arcpy.env.overwriteOutput=True
sr=arcpy.SpatialReference(32633)
def polygon(points):
 return arcpy.Polygon(arcpy.Array([arcpy.Point(500000+x,5000000+y) for x,y in points]),sr)
def box(x,y,w,h):return [(x,y),(x,y+h),(x+w,y+h),(x+w,y),(x,y)]
scenarios={
 'rectangles':[box(0,0,20,20),box(10,0,20,20)],
 'unequal':[box(0,0,40,20),box(10,0,20,20)],
 'oblique':[box(0,0,20,20),[(10,-5),(5,15),(30,25),(25,0),(10,-5)]],
 'contained':[box(0,0,40,40),box(10,10,10,10)],
 'three':[box(0,0,20,20),box(10,0,20,20),box(5,10,20,20)],
 'disjoint':[box(0,0,10,10),box(30,0,10,10)],
}
results={}
for name,shapes in scenarios.items():
 inputs=[]
 for i,points in enumerate(shapes):
  layer='memory/rom_input_'+str(i)
  arcpy.management.CreateFeatureclass('memory','rom_input_'+str(i),'POLYGON',spatial_reference=sr)
  arcpy.management.AddField(layer,'NAME','TEXT',field_length=8)
  with arcpy.da.InsertCursor(layer,['SHAPE@','NAME']) as c:c.insertRow((polygon(points),chr(65+i)))
  inputs.append(layer)
 for method in ['CENTER_LINE','THIESSEN','GRID']:
  try:
   arcpy.analysis.RemoveOverlapMultiple(inputs,'memory/rom_output',method,'ALL')
   fields=[f.name for f in arcpy.ListFields('memory/rom_output') if f.type not in ['Geometry','OID']]
   results[name+'_'+method]={'fields':[(f.name,f.type,f.length) for f in arcpy.ListFields('memory/rom_output')],
    'rows':list(arcpy.da.SearchCursor('memory/rom_output',fields+['SHAPE@JSON','SHAPE@AREA'])),'messages':arcpy.GetMessages()}
  except Exception as e:results[name+'_'+method]={'error':str(e),'messages':arcpy.GetMessages()}
 for layer in inputs:arcpy.management.Delete(layer)
with open(P('/tmp/unigis-rom-probe.json'),'w',newline='\n') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-rom-probe.json')
