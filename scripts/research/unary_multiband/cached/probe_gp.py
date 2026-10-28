import json
arcpy.env.overwriteOutput=True
base='/tmp/unigis-cached-int-multiband/'
for name in ['u32','u32_overflow','u32_band2','i32']:
 source=base+name+'.tif';out=base+name+'_Square_gp.tif'
 result=arcpy.gp.Square_sa(P(source),P(out))
 outputs=[result.getOutput(i) for i in range(result.outputCount)]
 print(json.dumps({'case':name,'outputs':outputs,'messages':result.getMessages()},default=str))
