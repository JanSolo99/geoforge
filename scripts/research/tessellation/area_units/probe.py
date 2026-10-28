import json,os
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-tess-units-controls';os.makedirs(P(base),exist_ok=True)
metres=arcpy.Describe(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')).spatialReference
feet=arcpy.SpatialReference(2272)
if not arcpy.Exists(P(base+'/feet_template.shp')):arcpy.management.CreateFeatureclass(P(base),'feet_template.shp','POLYGON',spatial_reference=feet)
base_extent='500000 5000000 500413 5000277'
cases=[('sqm','10000 SquareMeters',metres,base_extent),('km','0.01 SquareKilometers',metres,base_extent),('ha','1 Hectares',metres,base_extent),('acre',str(10000/4046.8564224)+' Acres',metres,base_extent),('sqft',str(10000/0.09290304)+' SquareFeet',metres,base_extent),('sqyard',str(10000/0.83612736)+' SquareYards',metres,base_extent),('sqmile',str(10000/2589988.110336)+' SquareMiles',metres,base_extent),('sqcm','100000000 SquareCentimeters',metres,base_extent),('are','100 Ares',metres,base_extent),('bare','10000',metres,base_extent),('feet_sqm','10000 SquareMeters',feet,base_extent),('feet_bare','10000',feet,base_extent),('feet_sqft','100000000 SquareFeet',feet,'500000 5000000 540000 5030000'),('feet_us','100000000 SquareFeetUS',feet,'500000 5000000 540000 5030000'),('unknown_sr','10000 SquareMeters',None,base_extent)]
for name,size,sr,extent in cases:
 try:
  out=base+'/'+name+'.shp'
  result=arcpy.management.GenerateTessellation(P(out),extent,'SQUARE',size,sr)
  count=int(arcpy.management.GetCount(P(out))[0]);rows=[]
  for grid,g in arcpy.da.SearchCursor(P(out),['GRID_ID','SHAPE@']):
   if len(rows)<2:rows.append({'grid':grid,'ring':[[p.X,p.Y] for p in g.getPart(0) if p],'area':g.area})
  print(json.dumps({'case':name,'size':size,'count':count,'wkid':arcpy.Describe(P(out)).spatialReference.factoryCode,'first':rows,'messages':result.getMessages()}))
 except Exception as error:print(json.dumps({'case':name,'error':str(error)}))
