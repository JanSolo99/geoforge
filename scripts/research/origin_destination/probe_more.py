"""ArcGIS follow-up: selection order, colors, numeric groups and centroid inputs."""
import json
arcpy.env.overwriteOutput = True
root = P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
sr = arcpy.SpatialReference(32633)
for name, kind, rows in [
 ('od_more_origins','POINT', [(0,0,'Z',30),(100,0,'A',10),(0,100,'M',20)]),
 ('od_more_destinations','POINT', [(300,300,'X',40),(0,100,'M',20),(110,0,'A',10),(30,40,'Z',30),(0,0,'Z',30)]),
 ('od_polygon','POLYGON',[(20,20,'Z',30)]),
 ('od_line','POLYLINE',[(20,20,'Z',30)]),
]:
 path=root+'/'+name+'.shp'
 arcpy.management.CreateFeatureclass(root,name+'.shp',kind,spatial_reference=sr)
 arcpy.management.AddField(path,'GRP','TEXT',field_length=8)
 arcpy.management.AddField(path,'NUM','LONG')
 with arcpy.da.InsertCursor(path,['SHAPE@','GRP','NUM']) as cursor:
  for x,y,group,num in rows:
   x+=500000;y+=5000000
   if kind=='POINT': g=arcpy.PointGeometry(arcpy.Point(x,y),sr)
   elif kind=='POLYGON':g=arcpy.Polygon(arcpy.Array([arcpy.Point(x,y),arcpy.Point(x+60,y),arcpy.Point(x+60,y+30),arcpy.Point(x,y+30),arcpy.Point(x,y)]),sr)
   else:g=arcpy.Polyline(arcpy.Array([arcpy.Point(x,y),arcpy.Point(x+60,y),arcpy.Point(x+60,y+30)]),sr)
   cursor.insertRow((g,group,num))
results={}
for name,origin,args in [
 ('nearest','od_more_origins',['','','PLANAR',1]),
 ('group','od_more_origins',['GRP','GRP']),
 ('numeric','od_more_origins',['NUM','NUM']),
 ('group_nearest','od_more_origins',['GRP','GRP','PLANAR',1]),
 ('only_one_group','od_more_origins',['GRP','GRP','PLANAR','',11]),
 ('polygon','od_polygon',[]),('line','od_line',[]),
 ('none','od_more_origins',['','','PLANAR','',0.001]),
 ('one_field','od_more_origins',['GRP']),
]:
 out=P('/tmp/unigis-od-more-'+name+'.shp')
 try:
  arcpy.analysis.GenerateOriginDestinationLinks(root+'/'+origin+'.shp',root+'/od_more_destinations.shp',out,*args)
  results[name]={'fields':[(f.name,f.type,f.length,f.precision,f.scale) for f in arcpy.ListFields(out)],'rows':list(arcpy.da.SearchCursor(out,['*','SHAPE@WKT'])),'messages':arcpy.GetMessages()}
 except Exception as e:results[name]={'error':str(e),'messages':arcpy.GetMessages()}
with open(P('/tmp/unigis-od-more-probe.json'),'w') as stream:json.dump(results,stream,indent=2)
print('Saved /tmp/unigis-od-more-probe.json')
