import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-copy-multicache-controls/'
for name in ['first_full','second_full','split_extrema','first_full_second_extrema','first_extrema_second_full','second_missing_extrema','domain_only','single_quotes']:
 try:
  result=arcpy.management.CopyRaster(P(base+name+'.tif'),P(base+name+'_out.tif'))
  print(json.dumps({'case':name,'ok':True,'messages':result.getMessages()}))
 except Exception as error: print(json.dumps({'case':name,'ok':False,'error':str(error)}))
