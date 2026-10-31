import numpy as np, itertools
from agl2 import agl
B='/tmp/vs2/'
def knn(Z,k,p,rad=3):
    H,W=Z.shape
    offs=[(i,j) for i in range(-rad+1,rad+1) for j in range(-rad+1,rad+1)]
    def f(px,py):
        px=np.asarray(px,float); py=np.asarray(py,float)
        x0=np.floor(px).astype(int); y0=np.floor(py).astype(int)
        ds=np.array([np.hypot(px-(x0+a),py-(y0+b)) for a,b in offs])
        vs=np.array([Z[np.clip(y0+b,0,H-1),np.clip(x0+a,0,W-1)] for a,b in offs])
        o=np.argsort(ds,axis=0,kind='stable')[:k]
        d=np.take_along_axis(ds,o,0); v=np.take_along_axis(vs,o,0)
        hit=d[0]<1e-9
        w=1/np.maximum(d,1e-12)**p
        return np.where(hit,v[0],(w*v).sum(0)/w.sum(0))
    return f
def radial(Z,r,p):
    H,W=Z.shape
    offs=[(i,j) for i in range(-2,4) for j in range(-2,4)]
    def f(px,py):
        px=np.asarray(px,float); py=np.asarray(py,float)
        x0=np.floor(px).astype(int); y0=np.floor(py).astype(int)
        num=0;den=0
        for a,b in offs:
            d=np.hypot(px-(x0+a),py-(y0+b)); w=np.where(d<r,np.maximum(r-d,0)**p,0)
            num=num+w*Z[np.clip(y0+b,0,H-1),np.clip(x0+a,0,W-1)]; den=den+w
        return num/den
    return f
if __name__=='__main__':
    probes=[]
    for n,col in [('p_agl2',24),('p_agl3',30)]:
        Z=np.zeros((41,41)); Z[20,col]=6; probes.append((Z,np.load(B+n+'_g.npy')))
    res=[]
    for mode,start in [('fixed',1),('fixed',2),('round',2),('ceil',2)]:
      for kname,mkf in [*[(f'knn{k}p{p}',lambda Z,k=k,p=p:knn(Z,k,p)) for k in (3,4,5,6,8,9,12,16) for p in (1,1.5,2)],*[(f'rad{r}p{p}',lambda Z,r=r,p=p:radial(Z,r,p)) for r in (1,1.2,1.5,2) for p in (1,2)]]:
        err=0
        for Z,G in probes:
            A=agl(Z,mkf(Z),(20,20),(20,20),mode,start); err+=np.abs(A-G)[12:29,20:41].mean()
        res.append((round(err,4),kname,mode,start))
    for r in sorted(res)[:12]: print(r)
