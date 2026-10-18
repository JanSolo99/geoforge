import arcpy, json
from arcpy.sa import *
arcpy.CheckOutExtension("Spatial"); arcpy.env.overwriteOutput=True
arcpy.env.extent=arcpy.env.snapRaster=arcpy.env.mask=None; arcpy.env.cellSize=None
D='/tmp/claude-1000/sc'; KP=P(D)+'/kp.shp'
out={}
for cs in (20,30,45):
    for rad in (60,90,150):
        try:
            r=PointDensity(KP,'NONE',cs,NbrCircle(rad,'MAP'),'SQUARE_METERS'); p=P(D+'/pdx.tif'); r.save(p); q=arcpy.Raster(p); e=q.extent
            out[f'{cs}_{rad}']=[e.XMin-500000,e.YMin-5000000,e.XMax-500000,e.YMax-5000000,q.width,q.height]
        except Exception as ex: out[f'{cs}_{rad}']=str(ex)[:100]
print('RES'+json.dumps(out))
