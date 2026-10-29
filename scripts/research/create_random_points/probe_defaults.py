"""Public default values and repeat-coordinate independence."""
import json
results={}
for i in [2,3,4,5,6,7]:
 try:results[str(i)]=str(arcpy.GetParameterValue('CreateRandomPoints_management',i))
 except Exception as e:results[str(i)]=str(e)
with open(P('/tmp/unigis-random-defaults.json'),'w') as f:json.dump(results,f,indent=2)
print(json.dumps(results,indent=2))
