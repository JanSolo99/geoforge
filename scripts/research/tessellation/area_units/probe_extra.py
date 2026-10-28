import json,os
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-tess-units-controls';os.makedirs(P(base),exist_ok=True)
metres=arcpy.Describe(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')).spatialReference
feet=arcpy.SpatialReference(2272)
if not arcpy.Exists(P(base+'/feet_template.shp')):arcpy.management.CreateFeatureclass(P(base),'feet_template.shp','POLYGON',spatial_reference=feet)
base_extent='500000 5000000 500413 5000277'
cases=[('sqmm','10000000000 SquareMillimeters',metres,base_extent),('sqdm','1000000 SquareDecimeters',metres,base_extent),('sqin',str(10000/0.00064516)+' SquareInches',metres,base_extent),('sqnaut',str(10000/(1852*1852))+' SquareNauticalMiles',metres,base_extent),('sqyard_us',str(10000/(9*(1200/3937)**2))+' SquareYardsUS',metres,base_extent),('acre_us',str(10000/(43560*(1200/3937)**2))+' AcresUS',metres,base_extent),('sqmile_us',str(10000/((5280*1200/3937)**2))+' SquareMilesUS',metres,base_extent),('sqin_us',str(10000/((100/3937)**2))+' SquareInchesUS',metres,base_extent)]
for name,size,sr,extent in cases:
 try:
  out=base+'/'+name+'.shp'
  result=arcpy.management.GenerateTessellation(P(out),extent,'SQUARE',size,sr)
  count=int(arcpy.management.GetCount(P(out))[0]);rows=[]
  for grid,g in arcpy.da.SearchCursor(P(out),['GRID_ID','SHAPE@']):
   if len(rows)<2:rows.append({'grid':grid,'ring':[[p.X,p.Y] for p in g.getPart(0) if p],'area':g.area})
  print(json.dumps({'case':name,'size':size,'count':count,'wkid':arcpy.Describe(P(out)).spatialReference.factoryCode,'first':rows,'messages':result.getMessages()}))
 except Exception as error:print(json.dumps({'case':name,'error':str(error)}))
