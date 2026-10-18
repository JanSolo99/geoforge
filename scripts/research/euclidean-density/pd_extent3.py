import arcpy, json
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial"); arcpy.env.overwriteOutput=True
arcpy.env.extent=arcpy.env.snapRaster=arcpy.env.mask=None; arcpy.env.cellSize=None
D='/tmp/claude-1000/sc'; sr=arcpy.SpatialReference(32632)
sets={'a':[(100,100),(400,100),(100,300),(400,300)],'b':[(100,100),(400,100),(100,300),(400,300),(250,200)],
 'c':[(100,100),(700,100),(100,300),(700,300)],'d':[(100,100),(400,100),(100,600),(400,600)],'e':[(100,100),(400,300)],
 'f':[(0,0),(300,300)], 'g':[(100,100),(400,100),(100,300),(400,300),(250,200),(300,150),(150,250)]}
out={}
for k,pts in sets.items():
    fc=arcpy.management.CreateFeatureclass(P(D),f'z{k}.shp','POINT',spatial_reference=sr)
    with arcpy.da.InsertCursor(fc,['SHAPE@XY']) as c:
        for x,y in pts: c.insertRow([(500000+x,5000000+y)])
    r=PointDensity(P(D)+f'/z{k}.shp','NONE',30,NbrCircle(90,'MAP'),'SQUARE_METERS'); p=P(D+'/pdx.tif'); r.save(p); q=arcpy.Raster(p); e=q.extent
    out[k]=[e.XMin-500000,e.YMin-5000000,e.XMax-500000,e.YMax-5000000,q.width,q.height]
print('RES'+json.dumps(out))
