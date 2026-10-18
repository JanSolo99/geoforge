import arcpy, json, random
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial"); arcpy.env.overwriteOutput=True
arcpy.env.extent=arcpy.env.snapRaster=arcpy.env.mask=None; arcpy.env.cellSize=None
D='/tmp/claude-1000/sc'; sr=arcpy.SpatialReference(32632)
out={}
for seed,n,w,h in [(1,12,430,340),(2,30,430,340),(3,5,200,600),(4,50,1000,1000)]:
    random.seed(seed)
    pts=[(500000+random.uniform(20,w),5000000+random.uniform(20,h)) for _ in range(n)]
    fc=arcpy.management.CreateFeatureclass(P(D),f'q{seed}.shp','POINT',spatial_reference=sr)
    with arcpy.da.InsertCursor(fc,['SHAPE@XY']) as c:
        for p in pts: c.insertRow([p])
    r=PointDensity(P(D)+f'/q{seed}.shp','NONE',30,NbrCircle(90,'MAP'),'SQUARE_METERS'); p=P(D+'/pdx.tif'); r.save(p); q=arcpy.Raster(p); e=q.extent
    xs=[a for a,b in pts]; ys=[b for a,b in pts]
    out[seed]=dict(n=n,dx=min(xs)-e.XMin,dy=min(ys)-e.YMin,w=max(xs)-min(xs),h=max(ys)-min(ys),cols=q.width,rows=q.height,ew=e.XMax-e.XMin,eh=e.YMax-e.YMin)
print('RES'+json.dumps(out))
