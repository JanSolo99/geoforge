import numpy as np, itertools
from p7 import interp
B='/tmp/vs2/'
def bil(Z):
    H,W=Z.shape
    def f(px,py):
        x0=np.clip(np.floor(px).astype(int),0,W-2); y0=np.clip(np.floor(py).astype(int),0,H-2)
        fx=px-x0; fy=py-y0
        return Z[y0,x0]*(1-fx)*(1-fy)+Z[y0,x0+1]*fx*(1-fy)+Z[y0+1,x0]*(1-fx)*fy+Z[y0+1,x0+1]*fx*fy
    return f
def agl(Z,g,ob,zo,mode='fixed',start=1,offset=1.0,cell=10,k=0.13,R=6371000.):
    H,W=Z.shape; ox,oy=ob
    z0=float(g(np.array(zo[0]),np.array(zo[1])))+offset
    yy,xx=np.mgrid[0:H,0:W].astype(float); dx,dy=xx-ox,yy-oy; L=np.hypot(dx,dy)
    if mode=='fixed': n=np.ceil(L-1e-9); ds=np.ones_like(L)
    else:
        n={'round':np.round(L),'ceil':np.ceil(L-1e-9),'floor':np.floor(L),'major':np.maximum(abs(dx),abs(dy))}[mode]; n=np.maximum(n,1); ds=L/n
    drop=lambda s:(s*cell)**2*(1-k)/(2*R)
    tz=Z-drop(L); need=np.full((H,W),-np.inf)
    for i in range(start,int(n.max())+1):
        s=i*ds; m=(s<L-1e-9)&(i<n+ (1 if mode=='fixed' else 0)); f=s/np.where(L>0,L,1)
        e=g(ox+dx*f,oy+dy*f)-drop(s)
        req=z0+(e-z0)/np.where(m,f,1)
        need=np.where(m,np.maximum(need,req),need)
    return np.maximum(0,need-tz)
if __name__=='__main__':
    Z=np.load(B+'p_agl1_z.npy'); G=np.load(B+'p_agl1_g.npy')
    interps={'bil':bil(Z),'idw4':interp(Z,'4',1),'idw4p2':interp(Z,'4',2),'idw9':interp(Z,'9',1),'idw16':interp(Z,'16',1)}
    res=[]
    for (iname,g),mode,start,ob,zo in itertools.product(interps.items(),['fixed','round','ceil','floor','major'],[1,2],[(20,20),(20.5,20.5)],[(20,20),(20.5,20.5)]):
        A=agl(Z,g,ob,zo,mode,start)
        res.append((float(np.abs(A-G)[15:26,15:41].mean()),iname,mode,start,ob,zo))
    for r in sorted(res)[:10]: print(r)
