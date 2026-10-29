"""Resolve CID behavior for FileGDB source versus shapefile source."""
import os,json
root=P('/tmp/unigis-spatial-sampling-formats');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0)
source=arcpy.management.CopyFeatures(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/ssl_areas.shp'),os.path.join(gdb,'areas')).getOutput(0)
results={}
for name,method,allocation in [('poly_equal','STRAT_POLY','EQUAL'),('poly_area','STRAT_POLY','PROP_AREA'),('poly_pop','STRAT_POLY','PROP_FIELD'),('id_equal','STRAT_ID','EQUAL'),('id_pop','STRAT_ID','PROP_FIELD')]:
 out=os.path.join(root,name+'.shp')
 try:
  arcpy.management.CreateSpatialSamplingLocations(source,out,sampling_method=method,strata_id_field='STRATUM',strata_count_method=allocation,num_samples=7,num_samples_per_strata=2,population_field='POP')
  results[name]=dict(cids=[r[0] for r in arcpy.da.SearchCursor(out,['CID'])],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-spatial-sampling-formats.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
