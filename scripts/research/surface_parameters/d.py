import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); Z=d["Z"].astype(np.float64); H,W=Z.shape; h=10.0
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=500000.0+(cols+0.5)*h; y=5001200.0-(rows+0.5)*h
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
X,Y,ZZ=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True).transform(lon,lat,Z)
P=np.stack([X,Y,ZZ],-1)
cells=[(r,c) for r in range(1,H-1,5) for c in range(1,W-1,5)]
def coefs(kind,w0):
    out=[]
    for r,c in cells:
        la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
        e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
        Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]
        xs,ys,zs=Q@e,Q@n,Q@u
        base=[xs*xs,ys*ys,xs*ys,xs,ys]
        if kind=="bi":
            A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys]+base+[np.ones(9)],1); co=np.linalg.solve(A,zs)[3:8]
        elif w0=="inf":
            A=np.stack(base,1); m=np.ones(9,bool); m[4]=False
            dd=np.hypot(xs,ys)[m]; w=np.sqrt(1/(dd/h)**4)
            co=np.linalg.lstsq(A[m]*w[:,None],zs[m]*w,rcond=None)[0]
        else:
            dd=np.hypot(xs,ys); w=np.where(dd<1e-6,w0,1/np.maximum(dd/h,1e-9)**4); w=np.sqrt(w)
            A=np.stack(base+[np.ones(9)],1); co=np.linalg.lstsq(A*w[:,None],zs*w,rcond=None)[0][:5]
        out.append(co)
    return np.array(out)
def params(co):
    a,b,s,p,q=co.T; r=2*a; t=2*b
    g=p*p+q*q; w=1+g
    prof=-(p*p*r+2*p*q*s+q*q*t)/(g*w**1.5)
    tang=-(q*q*r-2*p*q*s+p*p*t)/(g*w**0.5)
    plan=-(q*q*r-2*p*q*s+p*p*t)/g**1.5
    tors=(p*q*(r-t)-(p*p-q*q)*s)/(g*w)
    mean=-((1+q*q)*r-2*p*q*s+(1+p*p)*t)/(2*w**1.5)
    gaus=(r*t-s*s)/w**2
    cas=np.sqrt(np.maximum(2*mean**2-gaus,0))
    return dict(PROFILE_CURVATURE=prof,TANGENTIAL_CURVATURE=tang,CONTOUR_CURVATURE=plan,CONTOUR_GEODESIC_TORSION=tors,MEAN_CURVATURE=mean,GAUSSIAN_CURVATURE=gaus,CASORATI_CURVATURE=cas,
                SLOPE=np.degrees(np.arctan(np.sqrt(g))))
for kind,w0,name in [("bi",None,"BIQUADRATIC"),("q","inf","QUADRATIC"),("q",1.0,"QUADRATIC"),("q",4.0,"QUADRATIC"),("q",16.0,"QUADRATIC")]:
    pr=params(coefs(kind,w0)); print(kind,w0)
    for k,v in pr.items():
        ref=np.array([d[f"{k}_{name}"][r,c] for r,c in cells])
        rel=np.abs(v-ref)/np.maximum(np.abs(ref),1e-12)
        relneg=np.abs(-v-ref)/np.maximum(np.abs(ref),1e-12)
        print("  ",k, "rel50 %.2e rel99 %.2e | neg rel50 %.2e"%(np.median(rel),np.percentile(rel,99),np.median(relneg)))
