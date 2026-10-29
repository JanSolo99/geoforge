"""Default-tolerance controls at the dataset's millimetre scale."""
import json
arcpy.env.overwriteOutput=True
arcpy.env.parallelProcessingFactor='0'
sr=arcpy.SpatialReference(32633)
results={}
for name,coords in {'pair':[[0,0],[0.002,0]],'duplicates':[[0,0],[0,0],[0.0006,0]],'chain':[[i*0.0006,0] for i in range(10)]}.items():
 fc='memory/pi_default'
 arcpy.management.CreateFeatureclass('memory','pi_default','POINT',spatial_reference=sr)
 with arcpy.da.InsertCursor(fc,['SHAPE@XY']) as c:
  for x,y in coords:c.insertRow(((500000+x,5000000+y),))
 before=list(arcpy.da.SearchCursor(fc,['SHAPE@XY']))
 arcpy.analysis.PairwiseIntegrate(fc)
 results[name]={'xy_tolerance':arcpy.Describe(fc).spatialReference.XYTolerance,'before':before,'after':list(arcpy.da.SearchCursor(fc,['SHAPE@XY']))}
 arcpy.management.Delete(fc)
with open(P('/tmp/unigis-pi-default.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-pi-default.json')
