import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); refs=np.load(sys.argv[2]); cs=float(sys.argv[3]); Z=d["Z"].astype(np.float64); H,W=Z.shape; h=cs
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=500000.0+(cols+0.5)*h; y=5000000.0+H*h-(rows+0.5)*h
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
X,Y,ZZ=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True).transform(lon,lat,Z)
P=np.stack([X,Y,ZZ],-1)
cells=[(r,c) for r in range(1,H-1,5) for c in range(1,W-1,5)]
L=[]
for r,c in cells:
    la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
    e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
    Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]; L.append((Q@e,Q@n,Q@u))
ref=np.array([refs[f"cs{int(cs)}_MEAN_CURVATURE_QUADRATIC"][r,c] for r,c in cells])
refb=np.array([refs[f"cs{int(cs)}_MEAN_CURVATURE_BIQUADRATIC"][r,c] for r,c in cells])
def mean(co):
    a,b,s,p,q=co.T; r=2*a; t=2*b; w=1+p*p+q*q
    return -((1+q*q)*r-2*p*q*s+(1+p*p)*t)/(2*w**1.5)
def run(rc,bi=False):
    out=[]
    for xs,ys,zs in L:
        if bi:
            A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys,xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1); out.append(np.linalg.solve(A,zs)[3:8]); continue
        m=np.ones(9,bool); m[4]=False
        w=np.sqrt(np.array([rc,1,rc,1,1,rc,1,rc]))
        A=np.stack([xs*xs,ys*ys,xs*ys,xs,ys],1)[m]
        out.append(np.linalg.lstsq(A*w[:,None],zs[m]*w,rcond=None)[0])
    v=mean(np.array(out)); R=refb if bi else ref
    return np.median(np.abs(v-R)/np.maximum(abs(R),1e-12))
print("bi", run(0,True))
best=min(np.linspace(0.2,0.5,301), key=run); 
fine=min(np.linspace(best-0.001,best+0.001,201), key=run); print("best", fine, run(fine))
def run2(mode):
    out=[]
    for xs,ys,zs in L:
        dd2 = xs*xs+ys*ys if mode=="2d" else (xs*xs+ys*ys+zs*zs if mode=="3d" else None)
        if mode=="grid":
            g=np.array([2,1,2,1,0,1,2,1,2])*h*h; dd2=g
        w=np.sqrt(1/(1+dd2)**2)
        A=np.stack([xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1)
        out.append(np.linalg.lstsq(A*w[:,None],zs*w,rcond=None)[0][:5])
    v=mean(np.array(out))
    rel=np.abs(v-ref)/np.maximum(abs(ref),1e-12)
    return np.median(rel), np.percentile(rel,99)
for m in ["2d","3d","grid"]: print(m, run2(m))
