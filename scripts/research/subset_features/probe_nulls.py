"""Compare public source and subset NULL handling in both output formats."""
import os,json
root=P('/tmp/unigis-subset-nulls');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'control.gdb').getOutput(0)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/subset_base.gdb/points')
results={'immutable_source':list(arcpy.da.SearchCursor(source,['KEY','LABEL','SCORE']))}
fc=arcpy.management.CreateFeatureclass(gdb,'nullable','POINT',spatial_reference=arcpy.SpatialReference(32633)).getOutput(0)
for name,typ in [('KEY','LONG'),('TXT','TEXT'),('NUM','DOUBLE'),('INT','LONG')]:arcpy.management.AddField(fc,name,typ,field_length=20)
with arcpy.da.InsertCursor(fc,['KEY','TXT','NUM','INT','SHAPE@XY'],explicit=True) as c:
 for row in [(1,None,None,None),(2,'alpha',7.5,9),(3,None,None,None),(4,'',0.0,0),(5,None,None,None)]:c.insertRow([*row,(500000+row[0]*10,5000000)])
results['null_source']=list(arcpy.da.SearchCursor(fc,['KEY','TXT','NUM','INT']))
for name,input_,fields in [('original',source,['KEY','LABEL','SCORE']),('nullable',fc,['KEY','TXT','NUM','INT'])]:
 for fmt in ['shp','gdb']:
  out=os.path.join(root,name+'.shp') if fmt=='shp' else os.path.join(gdb,name+'_out')
  arcpy.management.SubsetFeatures(input_,out,'',100,'PERCENTAGE_OF_INPUT')
  results[name+'_'+fmt]=list(arcpy.da.SearchCursor(out,fields))
with open(P('/tmp/unigis-subset-nulls.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
