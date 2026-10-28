import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-copy-partial-controls/'
for name in ['minmax','meanstd','stale','all','missing_minimum','missing_maximum','missing_mean','missing_stddev','missing_count','extra','missing_mean_extra']:
 try:
  result=arcpy.management.CopyRaster(P(base+name+'.tif'),P(base+name+'_out.tif'))
  print(json.dumps({'case':name,'ok':True,'messages':result.getMessages()}))
 except Exception as error:
  print(json.dumps({'case':name,'ok':False,'error':str(error),'messages':arcpy.GetMessages()}))
