import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); Z=d["Z"]; H,W=Z.shape; h=10.0
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=500000.0+(cols+0.5)*h; y=5001200.0-(rows+0.5)*h
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
X,Y,ZZ=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True).transform(lon,lat,Z)
P=np.stack([X,Y,ZZ],-1)
ref=d["SLOPE_QUADRATIC"]
def local(r,c):
    la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
    e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
    Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]
    return Q@e,Q@n,Q@u
cells=[(r,c) for r in range(1,H-1,7) for c in range(1,W-1,7)]
L=[local(r,c) for r,c in cells]
def test(wfun,const=True):
    err=[]
    for (r,c),(xs,ys,zs) in zip(cells,L):
        w=np.sqrt(wfun(np.hypot(xs,ys)))
        A=[xs*xs,ys*ys,xs*ys,xs,ys]+([np.ones(9)] if const else [])
        A=np.stack(A,1)
        co=np.linalg.lstsq(A*w[:,None],zs*w,rcond=None)[0]
        err.append(np.degrees(np.arctan(np.hypot(co[3],co[4])))-ref[r,c])
    err=np.abs(err); return err.max(), np.median(err)
for k in [0,1,2,3,4]:
    print("1/d^k center1", k, test(lambda dd: np.where(dd<1e-6,1.0,1/np.maximum(dd/h,1e-9)**k)))
for s in [0.5,0.6,0.7,0.8,1.0]:
    print("gauss",s, test(lambda dd: np.exp(-(dd/h)**2/(2*s*s))))
print("noconst", test(lambda dd: np.ones_like(dd), False))
