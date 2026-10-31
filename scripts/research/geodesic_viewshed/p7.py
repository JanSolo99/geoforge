import numpy as np, pickle
B='/tmp/vs2/'
pr={}
for f in ['bprobes.pkl','bprobes2.pkl']:
    for k,v in pickle.load(open(B+f,'rb')).items():
        d,h=k.split('_'); Z=np.zeros((41,41)); Z[20,20+int(d)]=float(h); pr['b'+k]=(Z,v,20,20)
Z=np.zeros((41,41)); Z[20,25]=10; pr['bump']=(Z,np.load(B+'p_bump_vs.npy'),20,20)
zr=np.load(B+'p_rand_z.npy').astype(float)
pr['rand']=(zr,np.load(B+'p_rand_vs.npy'),20,20); pr['rand_off']=(zr,np.load(B+'p_rand_off_vs.npy'),19.8,20.3)
def interp(Z,nb,p):
    H,W=Z.shape
    def f(px,py):
        if nb=='4': offs=[(a,b) for a in (0,1) for b in (0,1)]; bx=np.floor(px); by=np.floor(py)
        elif nb=='9': offs=[(a,b) for a in (-1,0,1) for b in (-1,0,1)]; bx=np.round(px); by=np.round(py)
        elif nb=='16': offs=[(a,b) for a in (-1,0,1,2) for b in (-1,0,1,2)]; bx=np.floor(px); by=np.floor(py)
        bx=bx.astype(int); by=by.astype(int)
        ds=np.array([np.hypot(px-(bx+a),py-(by+b)) for a,b in offs])
        vs=np.array([Z[np.clip(by+b,0,H-1),np.clip(bx+a,0,W-1)] for a,b in offs])
        w=1/np.maximum(ds,1e-9)**p
        return (w*vs).sum(0)/w.sum(0)
    return f
def vs(Z,ox,oy,g,start=1,step=1,offset=1.0,cell=10,k=0.13,R=6371000.):
    H,W=Z.shape
    z0=Z[int(round(oy)),int(round(ox))]+offset
    yy,xx=np.mgrid[0:H,0:W].astype(float); dx,dy=xx-ox,yy-oy; L=np.hypot(dx,dy)
    drop=lambda s:(s*cell)**2*(1-k)/(2*R)
    tz=Z-drop(L); vis=np.ones((H,W),bool); s=start
    while s<L.max():
        m=(s<L-1e-9)&vis; f=s/np.where(L>0,L,1)
        e=g(ox+dx*f,oy+dy*f)-drop(s); line=z0+(tz-z0)*f
        vis&=~(m&(e>=line)); s+=step
    return vis.astype(int)
if __name__=='__main__':
  for nb in ['4','9','16']:
    for p in [1,2,3]:
      for start in [1,2]:
        res={}
        for n,(Z,a,ox,oy) in pr.items():
            b=vs(Z,ox,oy,interp(Z,nb,p),start=start); res[n]=int((a!=b).sum())
        print(nb,p,start,sum(res.values()),res)
