import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); Z=d["Z"].astype(np.float64); H,W=Z.shape; h=10.0
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=500000.0+(cols+0.5)*h; y=5001200.0-(rows+0.5)*h
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
ecef=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True)
ref=d["SLOPE_BIQUADRATIC"]; refa=d["ASPECT_BIQUADRATIC"]
def run(zscale=1.0, use_h=True, label=""):
    X,Y,ZZ=ecef.transform(lon,lat,Z*zscale if use_h else 0*Z)
    P=np.stack([X,Y,ZZ],-1)
    errs=[];sl=[]
    for r in range(1,H-1,3):
        for c in range(1,W-1,3):
            la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
            e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
            Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]
            xs,ys,zs=Q@e,Q@n,Q@u
            if not use_h: zs=(Z[r-1:r+2,c-1:c+2].ravel()-Z[r,c])
            A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys,xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)
            co=np.linalg.solve(A,zs)
            s=np.degrees(np.arctan(np.hypot(co[6],co[7])))
            errs.append(s-ref[r,c]); sl.append(ref[r,c])
    errs=np.array(errs); sl=np.array(sl)
    print(label, abs(errs).max(), np.median(errs), np.corrcoef(errs,sl)[0,1], np.median(errs/np.tan(np.radians(sl))))
run(label="h")
# exact planar ZT for comparison of distances only (grid spacing scaled by k)
X,Y,ZZ=ecef.transform(lon,lat,Z)
P=np.stack([X,Y,ZZ],-1)
E=np.full((H,W),np.nan)
for r in range(1,H-1):
    for c in range(1,W-1):
        la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
        e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
        Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]
        xs,ys,zs=Q@e,Q@n,Q@u
        A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys,xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)
        co=np.linalg.solve(A,zs)
        E[r,c]=np.degrees(np.arctan(np.hypot(co[6],co[7])))-ref[r,c]
a=np.abs(E)
print(np.nanpercentile(a,[50,90,99,99.9]), int((a>5e-5).sum()), int(np.isfinite(a).sum()))
idx=np.argwhere(a>5e-5); print(idx[:20].tolist())
r,c=idx[0]; print(Z[r-1:r+2,c-1:c+2], ref[r,c], E[r,c])
