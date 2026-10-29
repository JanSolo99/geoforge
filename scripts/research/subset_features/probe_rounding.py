"""Resolve percentage ties and very small valid percentages."""
import os,json
root=P('/tmp/unigis-subset-rounding');os.makedirs(root,exist_ok=False)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/subset_base.gdb/points')
five=arcpy.management.MakeFeatureLayer(source,'subset_five_probe','KEY >= 4').getOutput(0)
results={}
for name,input_,size,units in [('tie',five,50,'PERCENTAGE_OF_INPUT'),('tiny',source,1,'PERCENTAGE_OF_INPUT'),('tiny_positive',source,0.001,'PERCENTAGE_OF_INPUT'),('abs_fraction',source,1.9,'ABSOLUTE_VALUE')]:
 train=os.path.join(root,name+'_train.shp');test=os.path.join(root,name+'_test.shp')
 try:
  arcpy.management.SubsetFeatures(input_,train,test,size,units)
  results[name]=dict(status='succeeded',train=[r[0] for r in arcpy.da.SearchCursor(train,['KEY'])],test=[r[0] for r in arcpy.da.SearchCursor(test,['KEY'])],messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-subset-rounding.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
