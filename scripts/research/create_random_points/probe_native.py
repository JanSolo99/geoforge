"""Public ArcGIS reader validates native random outputs without coordinate equality."""
import os,json,collections
root=P('/mnt/media/ArcGIS_Linux');native=P('/tmp/unigis-random-native');reference=os.path.join(root,'testdata','arcgis-reference')
with open(os.path.join(reference,'manifest.json')) as f:cases=json.load(f)['cases']
def snapshot(path):
 d=arcpy.Describe(path)
 fields=[dict(name=f.name,type=f.type,length=f.length,precision=f.precision,scale=f.scale,nullable=f.isNullable,required=f.required,alias=f.aliasName) for f in arcpy.ListFields(path)]
 attr=[f.name for f in arcpy.ListFields(path) if f.type not in ['OID','Geometry']]
 rows=[]
 for row in arcpy.da.SearchCursor(path,attr+['SHAPE@']):
  g=row[-1];j=json.loads(g.JSON);points=[(j['x'],j['y'])] if d.shapeType=='Point' else [tuple(p[:2]) for p in j['points']]
  rows.append(dict(values=list(row[:-1]),points=points))
 return dict(shape=d.shapeType,crs=[d.spatialReference.factoryCode,d.spatialReference.name],fields=fields,rows=rows)
results={}
for name,case in cases.items():
 if not name.startswith('rnp_') or case['status']!='succeeded':continue
 path=os.path.join(native,name+'.shp');a=snapshot(os.path.join(reference,name+'.shp'));b=snapshot(path);p=case['run']['params'];errors=[]
 for key in ['shape','crs','fields']:
  if a[key]!=b[key]:errors.append(key)
 if [len(r['points']) for r in a['rows']]!=[len(r['points']) for r in b['rows']]:errors.append('counts/chunks')
 source=p.get('constraining_feature_class')
 if source:
  source=os.path.join(root,source);sr=arcpy.Describe(source).spatialReference
  constraints=list(arcpy.da.SearchCursor(source,['OID@','SHAPE@']))
  point_source=arcpy.Describe(source).shapeType in ['Point','Multipoint']
  def membership(snap):
   counts=collections.Counter();groups=collections.defaultdict(list)
   for row in snap['rows']:
    for xy in row['points']:
     geom=arcpy.PointGeometry(arcpy.Point(*xy),sr)
     eligible=[(oid,g) for oid,g in constraints if (snap['shape']=='Multipoint' or row['values'][0]==oid) and geom.distanceTo(g)<1e-7]
     assert eligible,'native point outside CID constraint'
     oid=eligible[0][0];counts[oid]+=1;groups[oid].append(geom)
   return counts,groups
  ca,ga=membership(a);cb,gb=membership(b)
  if not point_source and ca!=cb:errors.append('points per source')
  gap=p.get('minimum_allowed_distance','0')
  field=None
  try:distance=float(gap.split()[0])
  except ValueError:field=gap
  if field:
   distances=dict(arcpy.da.SearchCursor(source,['OID@',field]))
  else:distances={oid:distance for oid,g in constraints}
  for oid,geoms in gb.items():
   for i,g in enumerate(geoms):
    if any(g.distanceTo(q)+1e-8<distances[oid] for q in geoms[:i]):errors.append('spacing')
 else:
  extent=p.get('constraining_extent','0 0 250 250')
  try:bounds=list(map(float,extent.split()));assert len(bounds)==4
  except (ValueError,AssertionError):
   e=arcpy.Describe(os.path.join(root,extent)).extent;bounds=[e.XMin,e.YMin,e.XMax,e.YMax]
  for row in b['rows']:
   for x,y in row['points']:
    if not(bounds[0]<=x<=bounds[2] and bounds[1]<=y<=bounds[3]):errors.append('extent')
 results[name]=dict(matches=not errors,errors=errors,expected=a,actual=b)
 print(name,'PASS' if not errors else str(errors))
with open(P('/tmp/unigis-random-native.json'),'w') as f:json.dump(results,f,indent=2)
assert len(results)==14 and all(r['matches'] for r in results.values())
