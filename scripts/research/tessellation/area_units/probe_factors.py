import json,os
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-tess-units-controls'
sr=arcpy.Describe(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/parcels.shp')).spatialReference
for name,unit in [('mile_factor','SquareMiles'),('mile_us_factor','SquareMilesUS'),('acre_factor','Acres'),('acre_us_factor','AcresUS')]:
 out=base+'/'+name+'.shp'
 arcpy.management.GenerateTessellation(P(out),'0 0 1 1','SQUARE','1 '+unit,sr)
 with arcpy.da.SearchCursor(P(out),['SHAPE@']) as rows:g=next(rows)[0]
 print(json.dumps({'case':name,'unit':unit,'area':g.area,'ring':[[p.X,p.Y] for p in g.getPart(0) if p]}))
