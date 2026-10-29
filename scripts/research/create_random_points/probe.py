"""Public count/schema/constraint controls; random coordinates are not an exact oracle."""
import os,json
root=P('/tmp/unigis-random-probe');os.makedirs(root,exist_ok=False)
sr=arcpy.SpatialReference(32633)
areas=arcpy.management.CreateFeatureclass(root,'areas.shp','POLYGON',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(areas,'N','DOUBLE');arcpy.management.AddField(areas,'GAP','DOUBLE')
with arcpy.da.InsertCursor(areas,['SHAPE@','N','GAP']) as c:
 for x,n,d in [(0,3,2),(30,0,1),(60,2.6,3)]:
  ring=[(x,0),(x,20),(x+20,20),(x+20,0),(x,0)]
  c.insertRow([arcpy.Polygon(arcpy.Array([arcpy.Point(500000+a,5000000+b) for a,b in ring]),sr),n,d])
lines=arcpy.management.CreateFeatureclass(root,'lines.shp','POLYLINE',spatial_reference=sr).getOutput(0)
arcpy.management.AddField(lines,'N','LONG')
with arcpy.da.InsertCursor(lines,['SHAPE@','N']) as c:
 for y,n in [(0,3),(30,2)]:c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Point(500000,5000000+y),arcpy.Point(500020,5000000+y)]),sr),n])
points=arcpy.management.CreateFeatureclass(root,'points.shp','POINT',spatial_reference=sr).getOutput(0)
with arcpy.da.InsertCursor(points,['SHAPE@XY']) as c:
 for i in range(5):c.insertRow([(500000+i*10,5000000)])
for fc,name in [(areas,'areas'),(lines,'lines'),(points,'points')]:arcpy.management.CopyFeatures(fc,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/rnp_'+name+'.shp'))
results={}
controls=[('extent','','0 0 20 10',7,'0','POINT',10),('zero','','0 0 20 10',0,'0','POINT',10),('negative','','0 0 20 10',-2,'0','POINT',10),('areas',areas,'',2,'0','POINT',10),('field',areas,'','N','GAP','POINT',10),('lines',lines,'',3,'2 Meters','POINT',10),('line_field',lines,'','N','0','POINT',10),('points',points,'',3,'0','POINT',10),('points_over',points,'',10,'0','POINT',10),('multi_extent','','0 0 20 10',7,'0','MULTIPOINT',3),('multi_areas',areas,'',4,'0','MULTIPOINT',3),('impossible',areas,'',3,'100 Meters','POINT',10),('ignored_extent',areas,'0 0 1 1',1,'0','POINT',10),('extent_source','',areas,4,'0','POINT',10),('missing_extent','','',3,'0','POINT',10)]
for name,fc,extent,n,gap,multi,size in controls:
 try:
  out=os.path.join(root,name+'.shp');r=arcpy.management.CreateRandomPoints(root,name+'.shp',fc,extent,n,gap,multi,size)
  results[name]=dict(status='succeeded',fields=[dict(name=f.name,type=f.type,length=f.length,precision=f.precision) for f in arcpy.ListFields(out)],rows=[dict(cid=row[0],geometry=json.loads(row[1])) for row in arcpy.da.SearchCursor(out,['CID','SHAPE@JSON'])],messages=arcpy.GetMessages(),shape=arcpy.Describe(out).shapeType)
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
# Independent repetition demonstrates why coordinates cannot be compared exactly.
arcpy.management.CreateRandomPoints(root,'extent_repeat.shp','','0 0 20 10',7)
results['extent_repeat']=dict(rows=list(arcpy.da.SearchCursor(os.path.join(root,'extent_repeat.shp'),['SHAPE@XY'])))
with open(P('/tmp/unigis-random-probe.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps({k:dict(status=v.get('status'),count=len(v.get('rows',[])),fields=v.get('fields'),cids=[r.get('cid') for r in v.get('rows',[]) if isinstance(r,dict)],error=v.get('error')) for k,v in results.items()},indent=2))
