"""Public FileGDB output reference, with original aliases and NULLs."""
import os,json
root=P('/tmp/unigis-subset-gdb-reference');os.makedirs(root,exist_ok=False)
out=arcpy.management.CreateFileGDB(root,'outputs.gdb').getOutput(0)
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/subset_base.gdb')
results={}
for item in ['points','records']:
 train=os.path.join(out,item+'_train');test=os.path.join(out,item+'_test')
 arcpy.management.SubsetFeatures(os.path.join(source,item),train,test)
 results[item]=dict(train=[r[0] for r in arcpy.da.SearchCursor(train,['KEY'])],test=[r[0] for r in arcpy.da.SearchCursor(test,['KEY'])])
with open(P('/tmp/unigis-subset-gdb-reference.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
