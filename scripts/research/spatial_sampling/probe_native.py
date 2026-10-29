"""Public ArcGIS reader validates native stochastic contracts and exact grid geometry."""
import os,json,collections,math
root=P('/mnt/media/ArcGIS_Linux');native=P('/tmp/unigis-spatial-sampling-native-v2');reference=os.path.join(root,'testdata','arcgis-reference')
with open(os.path.join(reference,'manifest.json')) as f:cases=json.load(f)['cases']
def read(path):
 d=arcpy.Describe(path);fs=arcpy.ListFields(path);attrs=[f.name for f in fs if f.type not in ['OID','Geometry']]
 schema=[dict(name=f.name,type=f.type,length=f.length,precision=f.precision,scale=f.scale,nullable=f.isNullable,required=f.required,alias=f.aliasName) for f in fs]
 return dict(shape=d.shapeType,crs=[d.spatialReference.factoryCode,d.spatialReference.name],schema=schema,rows=[(list(r[:-1]),r[-1]) for r in arcpy.da.SearchCursor(path,attrs+['SHAPE@'])])
results={}
for name,case in cases.items():
 if not name.startswith('ssl_') or case['status']!='succeeded':continue
 a=read(os.path.join(reference,name+'.shp'));b=read(os.path.join(native,name+'.shp'));p=case['run']['params'];method=p.get('sampling_method','RANDOM');errors=[]
 for k in ['shape','crs','schema']:
  if a[k]!=b[k]:errors.append(k)
 if len(a['rows'])!=len(b['rows']):errors.append('row count')
 if method=='SYSTEMATIC':
  for (av,ag),(bv,bg) in zip(a['rows'],b['rows']):
   if any((abs(x-y)>1e-7*max(1,abs(x))) if isinstance(x,(int,float)) else x!=y for x,y in zip(av,bv)):errors.append('grid attributes')
   if a['shape']=='Point':
    if ag.distanceTo(bg)>0.001:errors.append('grid centroid')
   elif ag.symmetricDifference(bg).area>0.001*max(1,ag.length):errors.append('grid polygon')
 else:
  ca=collections.Counter(v[0] for v,g in a['rows']);cb=collections.Counter(v[0] for v,g in b['rows'])
  if ca!=cb:errors.append('allocation')
  source=os.path.join(root,p['input']);source_rows=list(arcpy.da.SearchCursor(source,['OID@','STRATUM','SHAPE@']))
  if method=='RANDOM':groups=[source_rows]
  elif method=='STRAT_POLY':groups=[[r] for r in source_rows]
  else:
   keys=sorted(set(r[1] for r in source_rows));groups=[[r for r in source_rows if r[1]==key] for key in keys]
  base=0 if p.get('strata_count_method','EQUAL') in ['PROP_AREA','PROP_FIELD'] else 1
  spacing=float(p.get('min_distance','0').split()[0]);seen=collections.defaultdict(list)
  for values,g in b['rows']:
   cid=values[0];index=0 if method=='RANDOM' else cid-base
   if not any(g.distanceTo(r[2])<1e-8 for r in groups[index]):errors.append('stratum membership')
   if any(g.distanceTo(previous)+1e-8<spacing for previous in seen[cid]):errors.append('spacing')
   seen[cid].append(g)
 def serial(d):return dict(shape=d['shape'],crs=d['crs'],schema=d['schema'],rows=[dict(values=v,geometry=json.loads(g.JSON)) for v,g in d['rows']])
 results[name]=dict(matches=not errors,errors=errors,expected=serial(a),actual=serial(b))
 print(name,'PASS' if not errors else str(errors))
a=read(P('/tmp/unigis-spatial-sampling-grid/within_small.shp'));b=read(os.path.join(native,'within_small.shp'));errors=[]
for key in ['shape','crs','schema']:
 if a[key]!=b[key]:errors.append(key)
if len(a['rows'])!=len(b['rows']):errors.append('row count')
for (av,ag),(bv,bg) in zip(a['rows'],b['rows']):
 if av!=bv:errors.append('within attributes')
 if ag.symmetricDifference(bg).area>0.001*max(1,ag.length):errors.append('within geometry')
results['within_small']=dict(matches=not errors,errors=errors,expected=serial(a),actual=serial(b))
print('within_small','PASS' if not errors else str(errors))
with open(P('/tmp/unigis-spatial-sampling-native-v2.json'),'w') as f:json.dump(results,f,indent=2)
assert len(results)==20 and all(r['matches'] for r in results.values())
