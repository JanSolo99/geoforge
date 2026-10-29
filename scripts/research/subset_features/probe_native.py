"""Public reader checks native partition contracts, including format-specific NULLs."""
import os,json,collections,datetime
root=P('/mnt/media/ArcGIS_Linux');native=P('/tmp/unigis-subset-native');reference=os.path.join(root,'testdata','arcgis-reference')
with open(os.path.join(reference,'manifest.json')) as f:cases=json.load(f)['cases']
primitive=['String','SmallInteger','Integer','BigInteger','Single','Double']
def snap(path):
 fs=arcpy.ListFields(path);d=arcpy.Describe(path);attrs=[f.name for f in fs if f.type not in ['OID','Geometry']];geometry=hasattr(d,'shapeType')
 schema=[]
 for f in fs:
  system=f.type in ['OID','Geometry'];name=f.name.upper() if system else f.name
  alias=f.aliasName.upper() if system and f.aliasName.lower()==f.name.lower() else f.aliasName
  schema.append(dict(name=name,alias=alias,type=f.type,length=f.length,precision=f.precision,scale=f.scale,nullable=f.isNullable,required=f.required,editable=f.editable))
 rows=[(dict(zip(attrs,row[:len(attrs)])),row[-1] if geometry else None) for row in arcpy.da.SearchCursor(path,attrs+(['SHAPE@'] if geometry else []))]
 return dict(schema=schema,rows=rows,shape=getattr(d,'shapeType',None),crs=d.spatialReference.factoryCode if geometry else None,raw_system_names=[f.name for f in fs if f.type in ['OID','Geometry']])
def serial(s):return dict(schema=s['schema'],shape=s['shape'],crs=s['crs'],raw_system_names=s['raw_system_names'],rows=[dict(values=v,geometry=json.loads(g.JSON) if g else None) for v,g in s['rows']])
def check(paths,oracles,source,dbf):
 full=snap(source);records={v['KEY']:(v,g) for v,g in full['rows']};types={f['name']:f['type'] for f in full['schema']};seen=[];errors=[];snapshots=[]
 for path,oracle in zip(paths,oracles):
  a=snap(oracle);b=snap(path);snapshots.append(dict(expected=serial(a),actual=serial(b)))
  for k in ['schema','shape','crs']:
   if a[k]!=b[k]:errors.append(k)
  if len(a['rows'])!=len(b['rows']):errors.append('count')
  previous={name:'' if ty=='String' else 0 for name,ty in types.items() if ty in primitive}
  order=[]
  for row,g in b['rows']:
   k=row['KEY'];seen.append(k);order.append(list(records).index(k));original,original_g=records[k];expected=dict(original)
   if dbf:
    for field,last in previous.items():
     if expected[field] is None:expected[field]=last
     else:previous[field]=expected[field]
    for field,ty in types.items():
     if ty=='String' and expected[field] is not None and not expected[field].strip():expected[field]=' '
   if row!=expected:errors.append('values')
   if bool(g)!=bool(original_g):errors.append('geometry nullability')
   elif g and g.distanceTo(original_g)>0.001:errors.append('geometry')
  if order!=sorted(set(order)):errors.append('input order')
 if len(set(seen))!=len(seen):errors.append('duplicate/overlap')
 if len(paths)==2 and set(seen)!=set(records):errors.append('coverage')
 return dict(matches=not errors,errors=errors,outputs=snapshots)
results={}
for name,case in cases.items():
 if not name.startswith('subf_') or case['status']!='succeeded':continue
 p=case['run']['params'];table=case['table_output'];ext='dbf' if table else 'shp';names=['training']+(['testing'] if p.get('out_test_feature_class') else [])
 results[name]=check([os.path.join(native,name,n+'.'+ext) for n in names],[os.path.join(reference,name,n+'.'+ext) for n in names],os.path.join(root,p['input']),True)
 print(name,'PASS' if results[name]['matches'] else str(results[name]['errors']))
for item in ['points','records']:
 name='gdb_'+item;names=[item+'_train',item+'_test'];results[name]=check([os.path.join(native,'outputs.gdb',n) for n in names],[P('/tmp/unigis-subset-gdb-reference/outputs.gdb/'+n) for n in names],os.path.join(root,'testdata','geoproc','subset_base.gdb',item),False)
 print(name,'PASS' if results[name]['matches'] else str(results[name]['errors']))
for name,source,oracle,ext in [('null_control',P('/tmp/unigis-subset-nulls/control.gdb/nullable'),P('/tmp/unigis-subset-nulls/nullable.shp'),'shp'),('null_dates',P('/tmp/unigis-subset-null-dates/source.gdb/dates'),P('/tmp/unigis-subset-null-dates/all.dbf'),'dbf')]:
 results[name]=check([os.path.join(native,name+'.'+ext)],[oracle],source,True);print(name,'PASS' if results[name]['matches'] else str(results[name]['errors']))
with open(P('/tmp/unigis-subset-native.json'),'w') as f:json.dump(results,f,indent=2,default=str)
assert len(results)==16 and all(r['matches'] for r in results.values())
