"""Public sampling schema/allocation controls; save the untouched source once."""
import os,json
root=P('/tmp/unigis-spatial-sampling-probe');os.makedirs(root,exist_ok=False)
sr=arcpy.SpatialReference(32633)
source=arcpy.management.CreateFeatureclass(root,'source.shp','POLYGON',spatial_reference=sr).getOutput(0)
for name,typ in [('STRATUM','TEXT'),('POP','LONG')]:arcpy.management.AddField(source,name,typ,field_length=12)
with arcpy.da.InsertCursor(source,['SHAPE@','STRATUM','POP']) as c:
 for x,w,s,pop in [(0,10,'A',2),(20,30,'A',3),(60,20,'B',5)]:
  g=arcpy.Polygon(arcpy.Array([arcpy.Point(500000+a,5000000+b) for a,b in [(x,0),(x,10),(x+w,10),(x+w,0),(x,0)]]),sr)
  c.insertRow([g,s,pop])
arcpy.management.CopyFeatures(source,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ssl_areas.shp'))
results={}
controls=[('default',{}),('random',dict(num_samples=7)),('poly_equal',dict(sampling_method='STRAT_POLY',num_samples_per_strata=3)),('poly_area',dict(sampling_method='STRAT_POLY',strata_count_method='PROP_AREA',num_samples=7)),('poly_field',dict(sampling_method='STRAT_POLY',strata_count_method='FIELD',population_field='POP')),('poly_pop',dict(sampling_method='STRAT_POLY',strata_count_method='PROP_FIELD',population_field='POP',num_samples=9)),('id_equal',dict(sampling_method='STRAT_ID',strata_id_field='STRATUM',num_samples_per_strata=2)),('id_area',dict(sampling_method='STRAT_ID',strata_id_field='STRATUM',strata_count_method='PROP_AREA',num_samples=7)),('id_field',dict(sampling_method='STRAT_ID',strata_id_field='STRATUM',strata_count_method='FIELD',population_field='POP')),('id_pop',dict(sampling_method='STRAT_ID',strata_id_field='STRATUM',strata_count_method='PROP_FIELD',population_field='POP',num_samples=9)),('spaced',dict(num_samples=6,min_distance='3 Meters')),('impossible',dict(num_samples=4,min_distance='100 Meters')),('zero',dict(num_samples=0)),('negative',dict(num_samples=-1)),('id_missing',dict(sampling_method='STRAT_ID')),('system_square',dict(sampling_method='SYSTEMATIC',bin_shape='SQUARE',bin_size='100 SquareMeters')),('system_polygon',dict(sampling_method='SYSTEMATIC',bin_shape='SQUARE',bin_size='100 SquareMeters',geometry_type='POLYGON'))]
for name,options in controls:
 out=os.path.join(root,name+'.shp')
 try:
  arcpy.management.CreateSpatialSamplingLocations(source,out,**options)
  d=arcpy.Describe(out);fields=[f.name for f in arcpy.ListFields(out) if f.type not in ['OID','Geometry']]
  results[name]=dict(status='succeeded',fields=[dict(name=f.name,type=f.type,length=f.length,precision=f.precision) for f in arcpy.ListFields(out)],shape=d.shapeType,rows=[dict(values=list(row[:-1]),geometry=json.loads(row[-1])) for row in arcpy.da.SearchCursor(out,fields+['SHAPE@JSON'])],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-spatial-sampling-probe.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps({k:dict(status=v['status'],fields=v.get('fields'),count=len(v.get('rows',[])),values=[r['values'] for r in v.get('rows',[])[:8]],error=v.get('error')) for k,v in results.items()},indent=2))
