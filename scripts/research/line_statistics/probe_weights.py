import numpy as np, math
arcpy.CheckOutExtension('Spatial')
arcpy.env.workspace=P('/tmp')
arcpy.env.overwriteOutput=True
D='/mnt/media/ArcGIS_Linux/testdata/geoproc/'
sr=arcpy.SpatialReference(32633)
lines=[r[0] for r in arcpy.da.SearchCursor(P(D+'roads.shp'),['SHAPE@'])]
ls=arcpy.sa.LineStatistics(P(D+'roads.shp'),'ROAD_ID',20,60,'LENGTH')
a=np.asarray(arcpy.RasterToNumPyArray(ls,nodata_to_value=-99)).astype(float); e=ls.extent
print(a.shape,(e.XMin,e.YMax),ls.meanCellWidth)
cells=[(r,c) for r in range(a.shape[0]) for c in range(a.shape[1]) if a[r,c]>0][:400]
import random; random.seed(1); sample=random.sample(cells,6)
def length_in(cx,cy,rad):
    circ=arcpy.PointGeometry(arcpy.Point(cx,cy),sr).buffer(rad)
    return sum(l.intersect(circ,2).length for l in lines if not l.disjoint(circ))
for (r,c) in sample:
    cx=e.XMin+(c+0.5)*20; cy=e.YMax-(r+0.5)*20
    print((r,c),round(a[r,c],3),[round(length_in(cx,cy,R),2) for R in (60,)], 'area-normalised', round(a[r,c]/ (math.pi*3600),6))

rows=[(r[0],r[1]) for r in arcpy.da.SearchCursor(P(D+'roads.shp'),['SHAPE@','ROAD_ID'])]
print([x[1] for x in rows])
def wsum(cx,cy,rad):
    circ=arcpy.PointGeometry(arcpy.Point(cx,cy),sr).buffer(rad)
    return sum(l.intersect(circ,2).length*i for l,i in rows if not l.disjoint(circ))
for (r,c) in sample:
    cx=e.XMin+(c+0.5)*20; cy=e.YMax-(r+0.5)*20
    print((r,c),round(a[r,c],3),round(wsum(cx,cy,60),3))
# more stats: MEAN, MAXIMUM etc quick check of semantics for MAXIMUM at those cells
for st in ['MAXIMUM','MEAN','VARIETY']:
    x=np.asarray(arcpy.RasterToNumPyArray(arcpy.sa.LineStatistics(P(D+'roads.shp'),'ROAD_ID',20,60,st),nodata_to_value=-99)).astype(float)
    print(st,[round(float(x[r,c]),3) for r,c in sample])
