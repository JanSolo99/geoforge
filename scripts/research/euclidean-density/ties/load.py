import numpy as np, math
from osgeo import gdal
T='/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/'
def rd(n):
    ds=gdal.Open(T+n); b=ds.GetRasterBand(1); return b.ReadAsArray().astype(float), b.GetNoDataValue(), ds.GetGeoTransform()
src,snd,gt=rd('streams_hydro.tif')
dirr,dnd,gt2=rd('eucdir_streams.tif')
dist,_,_=rd('eucdist_streams.tif')
R,C=src.shape
S=[(r,c) for r in range(R) for c in range(C) if src[r,c]==1]
Sarr=np.array(S)
def ang(r,c,sr,sc):
    if (r,c)==(sr,sc): return 0
    a=math.floor(math.degrees(math.atan2(sc-c,r-sr))%360+0.5)
    return 360 if a in (0,360) else a
def cands(r,c):
    d2=(Sarr[:,0]-r)**2+(Sarr[:,1]-c)**2
    m=d2.min(); idx=np.where(d2==m)[0]
    return m,[tuple(Sarr[i]) for i in idx]
