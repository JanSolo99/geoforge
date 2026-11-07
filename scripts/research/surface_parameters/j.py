import numpy as np, sys
from osgeo import gdal
from pyproj import Transformer
gdal.UseExceptions()
ds=gdal.Open('/mnt/media/ArcGIS_Linux/testdata/raster/dem_nodata.tif'); Z=ds.ReadAsArray().astype(np.float64); gt=ds.GetGeoTransform(); M=Z==-9999
o=np.load(sys.argv[1]); H,W=Z.shape
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=gt[0]+(cols+0.5)*gt[1]; y=gt[3]+(rows+0.5)*gt[5]
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
ecef=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True)
def fit(xs,ys,zs,bi,mask):
    if bi:
        A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys,xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)[mask]; co=np.linalg.lstsq(A,zs[mask],rcond=None)[0]; return co[6],co[7]
    w=np.sqrt(1/(1+xs*xs+ys*ys)**2)[mask]; A=np.stack([xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)[mask]
    co=np.linalg.lstsq(A*w[:,None],zs[mask]*w,rcond=None)[0]; return co[3],co[4]
for r in range(1,H-1):
    for c in range(1,W-1):
        win=M[r-1:r+2,c-1:c+2].ravel()
        if M[r,c] or not win.any() or np.isnan(o["nd_SLOPE_QUADRATIC"][r,c]): continue
        out=[]
        for mode in ["drop","centre","mirror"]:
            z=Z[r-1:r+2,c-1:c+2].ravel().copy()
            mask=~win
            if mode=="centre": z[win]=z[4]; mask=np.ones(9,bool)
            if mode=="mirror": z[win]=2*z[4]-z[::-1][win]; mask=np.ones(9,bool)
            zz=z.reshape(3,3)
            X,Y,ZZ=ecef.transform(lon[r-1:r+2,c-1:c+2],lat[r-1:r+2,c-1:c+2],zz)
            Pw=np.stack([X,Y,ZZ],-1).reshape(9,3)
            la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
            u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
            v=Pw[5]-Pw[4]; v=v-(v@u)*u; e=v/np.linalg.norm(v); n=np.cross(u,e)
            Q=Pw-Pw[4]; xs,ys,zs=Q@e,Q@n,Q@u
            for bi,name in [(False,"QUADRATIC"),(True,"BIQUADRATIC")]:
                p,q=fit(xs,ys,zs,bi,mask)
                out.append("%s/%s %.2e"%(mode,name[:2],np.degrees(np.arctan(np.hypot(p,q)))-o[f"nd_SLOPE_{name}"][r,c]))
        print((r,c), win.astype(int).tolist(), " ".join(out))
