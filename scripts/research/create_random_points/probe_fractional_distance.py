"""Resolve documented rounding versus linear-unit and field fractional spacing."""
import os,json,math
root=P('/tmp/unigis-random-distance');os.makedirs(root,exist_ok=False)
source=arcpy.management.CopyFeatures(P('/mnt/media/ArcGIS_Linux/testdata/geoproc/rnp_areas.shp'),os.path.join(root,'source.shp')).getOutput(0)
with arcpy.da.UpdateCursor(source,['GAP']) as c:
 for row in c:c.updateRow([0.6])
results={}
for name,gap in [('field','GAP'),('constant','0.6 Meters')]:
 arcpy.management.CreateRandomPoints(root,name+'.shp',source,'',100,gap)
 groups={}
 for cid,xy in arcpy.da.SearchCursor(os.path.join(root,name+'.shp'),['CID','SHAPE@XY']):groups.setdefault(cid,[]).append(xy)
 results[name]={str(cid):dict(count=len(ps),minimum=min(math.dist(a,b) for i,a in enumerate(ps) for b in ps[:i])) for cid,ps in groups.items()}
with open(P('/tmp/unigis-random-distance.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
