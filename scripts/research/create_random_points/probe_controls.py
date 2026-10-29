"""Corrected controls use immutable sources and distinct disposable output names."""
import os,json
root=P('/tmp/unigis-random-controls');os.makedirs(root,exist_ok=False)
base=P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
areas=os.path.join(base,'rnp_areas.shp');lines=os.path.join(base,'rnp_lines.shp');points=os.path.join(base,'rnp_points.shp')
controls=[('extent','','0 0 20 10',7,'0','POINT',10),('zero','','0 0 20 10',0,'0','POINT',10),('negative','','0 0 20 10',-2,'0','POINT',10),('areas',areas,'',2,'0','POINT',10),('field',areas,'','N','GAP','POINT',10),('lines',lines,'',3,'2 Meters','POINT',10),('line_field',lines,'','N','0','POINT',10),('points',points,'',3,'0','POINT',10),('points_over',points,'',10,'0','POINT',10),('multi_extent','','0 0 20 10',7,'0','MULTIPOINT',3),('multi_areas',areas,'',4,'0','MULTIPOINT',3),('impossible',areas,'',3,'100 Meters','POINT',10),('ignored_extent',areas,'0 0 1 1',1,'0','POINT',10),('extent_source','',areas,4,'0','POINT',10),('missing_extent','','',3,'0','POINT',10)]
results={}
for name,fc,extent,n,gap,multi,size in controls:
 try:
  out=os.path.join(root,name+'.shp');arcpy.management.CreateRandomPoints(root,name+'.shp',fc,extent,n,gap,multi,size)
  fields=[f.name for f in arcpy.ListFields(out) if f.type not in ['OID','Geometry']]
  results[name]=dict(status='succeeded',fields=[dict(name=f.name,type=f.type,length=f.length,precision=f.precision) for f in arcpy.ListFields(out)],rows=[dict(values=list(row[:-1]),geometry=json.loads(row[-1])) for row in arcpy.da.SearchCursor(out,fields+['SHAPE@JSON'])],messages=arcpy.GetMessages(),shape=arcpy.Describe(out).shapeType)
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
arcpy.management.CreateRandomPoints(root,'extent_repeat.shp','','0 0 20 10',7)
results['extent_repeat']=dict(rows=list(arcpy.da.SearchCursor(os.path.join(root,'extent_repeat.shp'),['SHAPE@XY'])))
with open(P('/tmp/unigis-random-controls.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps({k:dict(status=v.get('status'),count=len(v.get('rows',[])),fields=v.get('fields'),values=[r.get('values') for r in v.get('rows',[]) if isinstance(r,dict)],error=v.get('error')) for k,v in results.items()},indent=2))
