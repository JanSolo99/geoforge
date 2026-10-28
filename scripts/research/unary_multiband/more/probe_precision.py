import json
arcpy.env.overwriteOutput=True
for op in ['Log2','Ln']:
 out='/tmp/unigis-log2-precision/'+op+'.tif'
 result=getattr(arcpy.gp,op+'_sa')(P('/tmp/unigis-log2-precision/input.tif'),P(out))
 print(json.dumps({'op':op,'output':str(result.getOutput(0))}))
