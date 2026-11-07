import numpy as np, sys
from pyproj import Transformer
d=np.load(sys.argv[1]); o=np.load(sys.argv[2]); Z=d["Z"].astype(np.float64); H,W=Z.shape; h=10.0
cols,rows=np.meshgrid(np.arange(W),np.arange(H))
x=300000.0+(cols+0.5)*h; y=5001200.0-(rows+0.5)*h
lon,lat=Transformer.from_crs(32633,4326,always_xy=True).transform(x,y)
X,Y,ZZ=Transformer.from_crs("EPSG:4979","EPSG:4978",always_xy=True).transform(lon,lat,Z)
P=np.stack([X,Y,ZZ],-1)
def frame(r,c,mode):
    la,lo=np.radians(lat[r,c]),np.radians(lon[r,c])
    e=np.array([-np.sin(lo),np.cos(lo),0]); n=np.array([-np.sin(la)*np.cos(lo),-np.sin(la)*np.sin(lo),np.cos(la)]); u=np.array([np.cos(la)*np.cos(lo),np.cos(la)*np.sin(lo),np.sin(la)])
    if mode=="grid":
        # x toward the east neighbour (cell centres on the ellipsoid surface), projected on the tangent plane
        v=(P[r,c+1]-P[r,c]); v=v-(v@u)*u; e=v/np.linalg.norm(v); n=np.cross(u,e)
    return e,n,u
for mode in ["enu","grid"]:
    for s in ["QUADRATIC","BIQUADRATIC"]:
        es=[];ea=[]
        for r in range(1,H-1,6):
            for c in range(1,W-1,6):
                e,n,u=frame(r,c,mode)
                Q=P[r-1:r+2,c-1:c+2].reshape(9,3)-P[r,c]; xs,ys,zs=Q@e,Q@n,Q@u
                if s=="BIQUADRATIC":
                    A=np.stack([xs*xs*ys*ys,xs*xs*ys,xs*ys*ys,xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1); co=np.linalg.solve(A,zs); p,q=co[6],co[7]
                else:
                    w=np.sqrt(1/(1+xs*xs+ys*ys)**2); A=np.stack([xs*xs,ys*ys,xs*ys,xs,ys,np.ones(9)],1); co=np.linalg.lstsq(A*w[:,None],zs*w,rcond=None)[0]; p,q=co[3],co[4]
                es.append(np.degrees(np.arctan(np.hypot(p,q)))-o[f"SLOPE_{s}"][r,c])
                ea.append(((np.degrees(np.arctan2(-p,-q))%360)-o[f"ASPECT_{s}"][r,c]+180)%360-180)
        print(mode,s,"slope",np.median(np.abs(es)),"aspect med",np.median(ea), np.median(np.abs(ea)))
