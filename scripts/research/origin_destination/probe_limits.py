"""ArcGIS bridge: color assignment after eight origins, numeric limit validation."""
import json
arcpy.env.overwriteOutput=True
sr=arcpy.SpatialReference(32633)
arcpy.management.CreateFeatureclass('memory','od_many','POINT',spatial_reference=sr)
with arcpy.da.InsertCursor('memory/od_many',['SHAPE@XY']) as c:
 for i in range(12):c.insertRow(((500000+i,5000000),))
dest=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/od_destinations.shp')
results={}
for name,args in [('default',[]),('repeat',[]),('n0',['','','PLANAR',0]),('n_fraction',['','','PLANAR',1.5]),('n_negative',['','','PLANAR',-1]),('radius0',['','','PLANAR','',0]),('radius_negative',['','','PLANAR','',-1])]:
 try:
  arcpy.analysis.GenerateOriginDestinationLinks('memory/od_many',dest,'memory/od_out',*args)
  results[name]={'rows':list(arcpy.da.SearchCursor('memory/od_out',['ORIG_FID','DEST_FID','COLOR_ID','LINK_DIST'])),'messages':arcpy.GetMessages()}
 except Exception as e:results[name]={'error':str(e),'messages':arcpy.GetMessages()}
with open(P('/tmp/unigis-od-limits.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-od-limits.json')
