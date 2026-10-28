import json,os
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-tess-transverse-controls'
os.makedirs(P(base),exist_ok=True)
sr=arcpy.Describe(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')).spatialReference
cases=[('a','500000 5000000 500400 5000300','10000 SquareMeters'),('b','500000 5000000 500413 5000277','10000 SquareMeters'),('c','500037 5000011 500390 5000333','5000 SquareMeters'),('d','500100 5000200 501000 5000500','20000 SquareMeters'),('tiny','500000 5000000 500001 5000001','10000 SquareMeters'),('thin','500000 5000000 500400 5000001','10000 SquareMeters'),('hectare','500000 5000000 500400 5000300','1 Hectares'),('kilometer','500000 5000000 500400 5000300','0.01 SquareKilometers')]
for name,extent,size in cases:
 try:
  out=base+'/'+name+'.shp'
  result=arcpy.management.GenerateTessellation(P(out),extent,'TRANSVERSE_HEXAGON',size,sr)
  rows=[]
  for oid,grid,g in arcpy.da.SearchCursor(P(out),['OID@','GRID_ID','SHAPE@']):
   rows.append({'oid':oid,'grid':grid,'centre':[g.trueCentroid.X,g.trueCentroid.Y],'ring':[[p.X,p.Y] for p in g.getPart(0) if p],'area':g.area})
  print(json.dumps({'case':name,'extent':extent,'size':size,'count':len(rows),'first':rows[:6],'last':rows[-2:]}))
 except Exception as error:print(json.dumps({'case':name,'error':str(error)}))
