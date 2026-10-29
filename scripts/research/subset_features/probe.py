"""Public partition size/schema/identity controls; immutable source saved once."""
import os,json
root=P('/tmp/unigis-subset-probe-fixed');os.makedirs(root,exist_ok=False)
gdb=arcpy.management.CreateFileGDB(root,'source.gdb').getOutput(0);sr=arcpy.SpatialReference(32633)
for name in ['points','records','empty']:
 fc=arcpy.management.CreateTable(gdb,name).getOutput(0) if name=='records' else arcpy.management.CreateFeatureclass(gdb,name,'POINT',spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG',field_alias='Record key');arcpy.management.AddField(fc,'LABEL','TEXT',field_length=20,field_alias='Record label');arcpy.management.AddField(fc,'SCORE','DOUBLE')
 if name!='empty':
  fields=['KEY','LABEL','SCORE']+(['SHAPE@XY'] if name=='points' else [])
  with arcpy.da.InsertCursor(fc,fields) as c:
   for i in range(1,9):c.insertRow([i,None if i==3 else 'row '+str(i),i*1.5]+([(500000+i*10,5000000)] if name=='points' else []))
  with arcpy.da.UpdateCursor(fc,['KEY']) as c:
   for row in c:
    if row[0]==2:c.deleteRow()
arcpy.management.Copy(gdb,P('/mnt/media/ArcGIS_Linux/testdata/geoproc/subset_base.gdb'))
results={}
for name,item,size,units,test in [('default','points',None,None,True),('percent33','points',33.3,'PERCENTAGE_OF_INPUT',True),('abs3','points',3,'ABSOLUTE_VALUE',True),('abs36','points',3.6,'ABSOLUTE_VALUE',True),('zero','points',0,'ABSOLUTE_VALUE',True),('all','points',100,'PERCENTAGE_OF_INPUT',True),('too_many','points',99,'ABSOLUTE_VALUE',True),('negative','points',-1,'ABSOLUTE_VALUE',True),('over_percent','points',101,'PERCENTAGE_OF_INPUT',True),('train_only','points',3,'ABSOLUTE_VALUE',False),('records','records',50,'PERCENTAGE_OF_INPUT',True),('empty','empty',50,'PERCENTAGE_OF_INPUT',True)]:
 train=os.path.join(root,name+('_train.dbf' if item=='records' else '_train.shp'));other=os.path.join(root,name+('_test.dbf' if item=='records' else '_test.shp')) if test else ''
 try:
  arcpy.management.SubsetFeatures(os.path.join(gdb,item),train,other,size,units)
  def snap(path):
   return dict(fields=[dict(name=f.name,type=f.type,length=f.length,alias=f.aliasName) for f in arcpy.ListFields(path)],keys=[r[0] for r in arcpy.da.SearchCursor(path,['KEY'])])
  results[name]=dict(status='succeeded',train=snap(train),test=snap(other) if other else None,messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
results['defaults']={str(i):str(arcpy.GetParameterValue('SubsetFeatures_management',i)) for i in [3,4]}
with open(P('/tmp/unigis-subset-probe-fixed.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
