import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); Z=d["Z"]; H,W=Z.shape; h=10.0
X0,Y0=500000.0,5001200.0
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=X0+(cols+0.5)*h; y=Y0-(rows+0.5)*h
t=Transformer.from_crs(32633,4326,always_xy=True)
lon,lat=t.transform(x,y)
ecef=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True)
mode=sys.argv[2]
X,Y,ZZ=ecef.transform(lon,lat,Z if mode=="h" else np.zeros_like(Z))
P=np.stack([X,Y,ZZ],-1)
def run(bi,name):
    sl=np.full((H,W),np.nan); asp=np.full((H,W),np.nan)
    for r in range(1,H-1):
        for c in range(1,W-1):
            la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
            e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
            Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]
            if mode=="0":
                Q=Q.copy(); xs=Q@e; ys=Q@n; zs=Z[r-1:r+2,c-1:c+2].ravel()-Z[r,c]
            else:
                xs=Q@e; ys=Q@n; zs=Q@u
            A=np.stack([xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)
            if bi: A=np.hstack([np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys],1),A])
            co=np.linalg.lstsq(A,zs,rcond=None)[0][-6:]
            dd,ee=co[3],co[4]
            sl[r,c]=np.degrees(np.arctan(np.hypot(dd,ee)))
            asp[r,c]=np.degrees(np.arctan2(-dd,-ee))%360
    ref=d[f"SLOPE_{name}"]; ra=d[f"ASPECT_{name}"]
    da=(asp-ra+180)%360-180
    print(name, np.nanmax(abs(sl-ref)), np.nanpercentile(abs(sl-ref),50), np.nanmax(abs(da)))
run(False,"QUADRATIC"); run(True,"BIQUADRATIC")
