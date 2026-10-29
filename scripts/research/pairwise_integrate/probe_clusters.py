"""Independent cluster thresholds and dense-chain controls."""
import json
arcpy.env.overwriteOutput=True
arcpy.env.parallelProcessingFactor='0'
sr=arcpy.SpatialReference(32633)
cases={'pair_'+str(d):[[0,0],[d,0]] for d in [0.9999,1,1.0001,1.2,1.4141,1.4143,1.5,2]}
cases.update({'chain_'+str(n):[[i*0.6,0] for i in range(n)] for n in [4,5,6,10]})
cases['duplicates']=[[0,0],[0,0],[0.6,0]]
cases['square']=[[0,0],[0.8,0],[0.8,0.8],[0,0.8]]
results={}
for name,points in cases.items():
 fc='memory/pi_cluster'
 arcpy.management.CreateFeatureclass('memory','pi_cluster','POINT',spatial_reference=sr)
 with arcpy.da.InsertCursor(fc,['SHAPE@XY']) as c:
  for x,y in points:c.insertRow(((500000+x,5000000+y),))
 arcpy.analysis.PairwiseIntegrate(fc,'1 Meters')
 results[name]={'input':points,'output':list(arcpy.da.SearchCursor(fc,['SHAPE@XY']))}
 arcpy.management.Delete(fc)
with open(P('/tmp/unigis-pi-clusters.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-pi-clusters.json')
