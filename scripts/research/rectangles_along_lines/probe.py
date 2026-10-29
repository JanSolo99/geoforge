"""Public rectangle-placement controls and immutable shapefile fixtures."""
import os,json
root=P('/tmp/unigis-rectangles-probe');os.makedirs(root,exist_ok=False)
sr=arcpy.SpatialReference(32633)
shapes={
 'horizontal':[[[(0,0),(100,0)]]],
 'vertical':[[[(0,0),(0,100)]]],
 'diagonal':[[[(0,0),(60,80)]]],
 'reverse':[[[(100,0),(0,0)]]],
 'bend':[[[(0,0),(50,0),(50,50)]]],
 'connected':[[[(0,0),(50,0)]],[[(50,0),(100,0)]]],
 'disconnected':[[[(0,0),(50,0)]],[[(100,100),(150,100)]]],
 'multipart':[[[(0,0),(50,0)],[(100,100),(150,100)]]],
 'branch':[[[(0,0),(50,0)]],[[(50,0),(100,0)]],[[(50,0),(50,50)]]],
 'short':[[[(0,0),(10,0)]]],
 'closed':[[[(0,0),(50,0),(50,50),(0,50),(0,0)]]],
}
results={}
for name,features in shapes.items():
 fc=arcpy.management.CreateFeatureclass(root,name+'.shp','POLYLINE',spatial_reference=sr).getOutput(0)
 with arcpy.da.InsertCursor(fc,['SHAPE@']) as c:
  for parts in features:
   c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Array([arcpy.Point(500000+x,5000000+y) for x,y in part]) for part in parts]),sr)])
 arcpy.management.CopyFeatures(fc,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ral_'+name+'.shp'))
 for sort in ['UR','UL','LR','LL'] if name in ['horizontal','bend','disconnected'] else ['UR']:
  out=os.path.join(root,name+'_'+sort+'.shp')
  try:
   arcpy.management.GenerateRectanglesAlongLines(fc,out,'30 Meters','20 Meters',sort)
   fields=[f.name for f in arcpy.ListFields(out) if f.type not in ['OID','Geometry']]
   results[name+'_'+sort]=dict(status='succeeded',fields=fields,rows=[dict(values=row[:-1],geometry=json.loads(row[-1])) for row in arcpy.da.SearchCursor(out,fields+['SHAPE@JSON'])],messages=arcpy.GetMessages())
  except Exception as e:results[name+'_'+sort]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
# Length defaults and a longer-than-line rectangle.
for name,source,along,perp in [('defaults','horizontal','',''),('long','horizontal','200 Meters',''),('partial_width','horizontal','30 Meters',''),('negative','horizontal','-1 Meters','20 Meters'),('zero','horizontal','0 Meters','20 Meters')]:
 try:
  out=os.path.join(root,name+'.shp');arcpy.management.GenerateRectanglesAlongLines(os.path.join(root,source+'.shp'),out,along,perp)
  fields=[f.name for f in arcpy.ListFields(out) if f.type not in ['OID','Geometry']]
  results[name]=dict(status='succeeded',fields=fields,rows=[dict(values=row[:-1],geometry=json.loads(row[-1])) for row in arcpy.da.SearchCursor(out,fields+['SHAPE@JSON'])],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-rectangles-probe.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps({k:dict(status=v['status'],count=len(v.get('rows',[])),first=v.get('rows',[])[:1],error=v.get('error')) for k,v in results.items()},indent=2))
