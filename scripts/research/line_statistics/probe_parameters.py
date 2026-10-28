import json
arcpy.CheckOutExtension('Spatial')
arcpy.env.workspace=P('/tmp')
arcpy.env.overwriteOutput=True
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/roads.shp')
for cell,radius in [(20,-1),(20,0),(-1,60),(0,60),(20,60)]:
 try:
  r=arcpy.sa.LineStatistics(source,'ROAD_ID',cell,radius,'MEAN')
  print(json.dumps({'cell':cell,'radius':radius,'status':'success','size':[r.width,r.height]}))
 except Exception as e:
  print(json.dumps({'cell':cell,'radius':radius,'status':'error','message':str(e)}))
