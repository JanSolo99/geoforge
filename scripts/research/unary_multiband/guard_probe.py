import json
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
base='/tmp/unigis-unary-guard-controls/'
for name,op in [('sqrt','SquareRoot'),('ln','Ln'),('exp','Exp')]:
 out=base+name+'.tif'
 try:
  getattr(arcpy.sa,op)(P(base+'float64.tif')).save(P(out))
  raster=arcpy.Raster(P(out))
  print(json.dumps({'op':op,'bands':raster.bandCount,'type':raster.pixelType,'nodata':raster.noDataValue}))
 except Exception as error:print(json.dumps({'op':op,'error':str(error)}))
