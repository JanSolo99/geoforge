import arcpy, os, json, glob
arcpy.env.overwriteOutput=True
REDUMP=False
D='/tmp/claude-1000/-mnt-media-ArcGIS-Linux/f97f1020-a076-4ba9-a902-4fe4816049f3/scratchpad/r2p'
names=[l.strip() for l in open(P(D+'/todo.txt')) if l.strip()]
for n in names:
    src=P(D+'/tif/'+n+'.tif'); outshp=P(D+'/out/'+n+'_s.shp')
    try:
        if not (REDUMP and arcpy.Exists(outshp)): arcpy.conversion.RasterToPolygon(src,outshp,"SIMPLIFY","Value")
        res=[]
        with arcpy.da.SearchCursor(outshp,['gridcode','SHAPE@']) as cur:
            for g,shp in cur:
                rings=json.loads(shp.JSON)['rings']
                res.append({'gridcode':g,'rings':rings})
        json.dump(res,open(P(D+'/out/'+n+'.json'),'w'))
        print('ok',n,len(res))
    except Exception as e:
        print('FAIL',n,e)
