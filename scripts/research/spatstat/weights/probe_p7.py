import arcpy, os
arcpy.env.overwriteOutput = True
S='/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/w7'
os.makedirs(S,exist_ok=True); D=P(S)
sr=arcpy.SpatialReference(32632)
def mk(name,kind):
    return arcpy.management.CreateFeatureclass(D,name+'.shp',kind,spatial_reference=sr)
def run(order, vertex=False, off=0):
    ln=mk('l','POLYLINE')
    with arcpy.da.InsertCursor(ln,['SHAPE@']) as c:
        pts=[(500000,5000000),(500050,5000000),(500100,5000000)] if vertex else [(500000,5000000),(500100,5000000)]
        c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Point(*p) for p in pts]))])
    pt=mk('p','POINT')
    with arcpy.da.InsertCursor(pt,['SHAPE@XY']) as c:
        for x in order: c.insertRow([(500000+x,5000000+off)])
    o=D+'/o.shp'
    arcpy.management.SplitLineAtPoint(ln,pt,o,'10 Meters')
    res=[]
    with arcpy.da.SearchCursor(o,['SHAPE@','ORIG_SEQ']) as cur:
        for g,s in cur: res.append((s,round(g.firstPoint.X-500000,2),round(g.lastPoint.X-500000,2)))
    print(order,vertex,off,res)
run([10,20,30,40])
run([40,30,20,10])
run([20,40,10,30])
run([10,20,30,40],True)
run([10,50,30,40],True,5)
run([10,20,30,40,60,70],True,3)
