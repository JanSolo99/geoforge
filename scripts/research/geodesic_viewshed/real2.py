import numpy as np
from osgeo import gdal
from k import knn
gdal.UseExceptions()
d=gdal.Open('/mnt/media/ArcGIS_Linux/.worktrees/lane-claude/testdata/raster/dem.tif'); Z=d.ReadAsArray().astype(float); t=d.GetGeoTransform()
H,W=Z.shape
def bil(px,py):
    x0=int(np.floor(px)); y0=int(np.floor(py)); fx=px-x0; fy=py-y0
    return Z[y0,x0]*(1-fx)*(1-fy)+Z[y0,x0+1]*fx*(1-fy)+Z[y0+1,x0]*(1-fx)*fy+Z[y0+1,x0+1]*fx*fy
def vis(g,ox,oy,scheme,offset=1.0,cell=10,k=0.13,R=6371000.):
    z0=bil(ox+.5,oy+.5)+offset
    yy,xx=np.mgrid[0:H,0:W].astype(float); dx,dy=xx-ox,yy-oy; L=np.hypot(dx,dy)
    drop=lambda s:(s*cell)**2*(1-k)/(2*R)
    tz=Z-drop(L); v=np.ones((H,W),bool)
    for i in range(0,200):
        if scheme=='fwd': s=i+0*L
        elif scheme=='major': n=np.maximum(abs(dx),abs(dy)); s=i*L/np.maximum(n,1)
        elif scheme=='round': n=np.maximum(np.round(L),1); s=i*L/n
        m=(s>1.5)&(s<L-1e-9)&v; f=s/np.where(L>0,L,1)
        if not m.any() and i>2: break
        e=g(ox+dx*f,oy+dy*f)-drop(s); line=z0+(tz-z0)*f
        v&=~(m&(e>=line))
    return v.astype(int)
for name,obs in [('hill',[(500405,5000895)]),('two',[(500405,5000895),(500155,5000305)])]:
    a=gdal.Open(f'/tmp/vs2/{name}.tif').ReadAsArray().astype(int); a[a==255]=0
    for kk in (6,7):
      for scheme in ['fwd','major','round']:
        b=sum(vis(knn(Z,kk,1),(x-t[0])/t[1]-.5,(y-t[3])/t[5]-.5,scheme) for x,y in obs)
        print(name,kk,scheme,(a!=b).sum(),(a>b).sum(),(a<b).sum())
