import sys, math
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
dem = gdal.Open(sys.argv[1]).ReadAsArray().astype(np.float64)
ref = gdal.Open(sys.argv[2]).ReadAsArray().astype(np.float64)
cs = 10.0
h, w = dem.shape
def normals(z):
    zp = np.pad(z, 1, mode='edge')
    # 3x3 Horn-style gradients
    a,b,c = zp[:-2,:-2], zp[:-2,1:-1], zp[:-2,2:]
    d,f = zp[1:-1,:-2], zp[1:-1,2:]
    g,hh,i = zp[2:,:-2], zp[2:,1:-1], zp[2:,2:]
    dzdx = ((c+2*f+i)-(a+2*d+g))/(8*cs); dzdy = ((g+2*hh+i)-(a+2*b+c))/(8*cs)
    n = np.stack([-dzdx, dzdy, np.ones_like(z)], -1)
    return n/np.linalg.norm(n, axis=-1, keepdims=True)
def smooth_normals(n, radius, thr):
    ct = math.cos(math.radians(thr)); out = np.zeros_like(n)
    r = int(radius)
    offs = [(dy,dx) for dy in range(-r,r+1) for dx in range(-r,r+1) if dy*dy+dx*dx <= radius*radius]
    acc = np.zeros_like(n); wsum = np.zeros((h,w))
    npd = np.pad(n, ((r,r),(r,r),(0,0)), mode='edge')
    for dy,dx in offs:
        nb = npd[r+dy:r+dy+h, r+dx:r+dx+w]
        cosang = (nb*n).sum(-1)
        wgt = np.where(cosang > ct, (cosang-ct)**2, 0.0)
        acc += nb*wgt[...,None]; wsum += wgt
    out = acc/wsum[...,None]
    return out/np.linalg.norm(out, axis=-1, keepdims=True)
def update(z, n, iters, maxch, z0):
    for _ in range(iters):
        zp = np.pad(z, 1, mode='edge'); acc=np.zeros_like(z); ws=np.zeros_like(z)
        np_ = np.pad(n, ((1,1),(1,1),(0,0)), mode='edge')
        for dy in (-1,0,1):
            for dx in (-1,0,1):
                if dy==0 and dx==0: continue
                zj = zp[1+dy:1+dy+h, 1+dx:1+dx+w]; nj = np_[1+dy:1+dy+h, 1+dx:1+dx+w]
                cosang=(nj*n).sum(-1); wgt=np.where(cosang>math.cos(math.radians(15)),(cosang-math.cos(math.radians(15)))**2,0)
                # plane through neighbour with the cell's normal
                zz = zj - (n[...,0]*(-dx*cs) + n[...,1]*(dy*cs))/n[...,2]
                acc += wgt*zz; ws += wgt
        znew = np.where(ws>0, acc/ws, z)
        z = np.where(np.abs(znew - z0) > maxch, z0, znew)
    return z
n = smooth_normals(normals(dem), 5, 15)
for iters in (3,):
    for maxch in (0.5, 1e9):
        z = update(dem.copy(), n, iters, maxch, dem)
        d = np.abs(z-ref)
        print(iters, maxch, 'max', d.max().round(4), 'mean', d.mean().round(5), 'p99', np.percentile(d,99).round(4))
print('baseline (no smoothing) mean', np.abs(dem-ref).mean().round(5))
