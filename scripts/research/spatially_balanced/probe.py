"""Public RRQRR design/seed controls; all outputs remain disposable."""
import os,json,numpy as np
root=P('/tmp/unigis-balanced-probe');os.makedirs(root,exist_ok=False)
sr=arcpy.SpatialReference(32633)
arrays={'uniform':np.ones((16,16),dtype=np.float32),'weighted':np.tile(np.linspace(0.05,1,16,dtype=np.float32),(16,1)),'sparse':np.eye(16,dtype=np.float32),'invalid':np.full((4,4),1.5,dtype=np.float32)}
inputs={}
for name,data in arrays.items():
 path=os.path.join(root,name+'.tif');arcpy.NumPyArrayToRaster(data,arcpy.Point(500000,5000000),10,10).save(path);arcpy.management.DefineProjection(path,sr);inputs[name]=path
results={}
for name,source,n,seed in [('uniform4','uniform',4,23),('uniform8','uniform',8,23),('repeat4','uniform',4,23),('weighted','weighted',8,23),('sparse','sparse',4,23),('invalid','invalid',4,23),('zero','uniform',0,23)]:
 arcpy.env.randomGenerator=f'{seed} MERSENNE_TWISTER';out=os.path.join(root,name+'.shp')
 try:
  arcpy.management.CreateSpatiallyBalancedPoints(inputs[source],n,out)
  fs=[f.name for f in arcpy.ListFields(out) if f.type not in ['OID','Geometry']]
  results[name]=dict(status='succeeded',fields=[dict(name=f.name,type=f.type,length=f.length) for f in arcpy.ListFields(out)],rows=list(arcpy.da.SearchCursor(out,fs+['SHAPE@XY'])),messages=arcpy.GetMessages())
 except Exception as e:results[name]=dict(status='failed',error=str(e),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-balanced-probe.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
