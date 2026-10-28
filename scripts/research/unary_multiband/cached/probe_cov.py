import json
arcpy.env.overwriteOutput=True
for name in ['u32_three_bands','u32_single_overflow']:
 base='/tmp/unigis-cached-int-multiband/';source=base+name+'.tif'
 arcpy.management.CalculateStatistics(P(source))
 for op in ['Abs','Negate','Square']:
  out=base+name+'_'+op+'_gp.tif'
  result=getattr(arcpy.gp,op+'_sa')(P(source),P(out))
  print(json.dumps({'case':name,'op':op,'outputs':[result.getOutput(i) for i in range(result.outputCount)],'messages':result.getMessages()},default=str))
