import arcpy, os
arcpy.env.overwriteOutput = True
S='/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/w8'
os.makedirs(S,exist_ok=True); D=P(S)
G='/mnt/media/ArcGIS_Linux/testdata/geoproc/'
arcpy.management.MakeFeatureLayer(P(G+'roads.shp'),'rd',"ROAD_ID = 4")
def run(ids):
    arcpy.management.MakeFeatureLayer(P(G+'wells.shp'),'wl',"WELL_ID IN (%s)"%','.join(map(str,ids)))
    o=D+'/o.shp'
    arcpy.management.SplitLineAtPoint('rd','wl',o,'20 Meters')
    res=[]
    with arcpy.da.SearchCursor(o,['SHAPE@','ORIG_SEQ']) as cur:
        for g,s in cur: res.append((s,round(g.firstPoint.X-500000,1),round(g.lastPoint.X-500000,1)))
    print(ids,res)
run([7,13]); run([25,31]); run([13,25]); run([31,13,7]); run([7,13,25]); run([7,31]); run([13,31]);run([25,13,7]); run([25,7])
