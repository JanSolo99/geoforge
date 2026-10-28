import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-copy-multicache-controls/'
for name in ['first_full','second_full','single_quotes']:
 for op in ['Abs','Negate','Square']:
  source=base+name+'_f64.tif';out=base+name+'_'+op+'.tif'
  getattr(arcpy.sa,op)(P(source)).save(P(out))
  print(json.dumps({'case':name,'operation':op,'type':arcpy.Raster(P(out)).pixelType}))
