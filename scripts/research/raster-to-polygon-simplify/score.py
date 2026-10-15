"""Compare simplify.py against ArcGIS outputs in out/<case>.json.
usage: python score.py [case ...]   (default: all cases with an out/*.json and a tif)"""
import sys, json, glob, os, time
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
import simplify
from simplify import simplify_polygons
import os as _os
if _os.environ.get('TIE'): simplify.TIE=_os.environ['TIE']
if _os.environ.get('TOL'): simplify.TOL=float(_os.environ['TOL'])
if _os.environ.get('SPLIT'): simplify.SPLIT=_os.environ['SPLIT']
if _os.environ.get('CHORD'): simplify.CHORD=_os.environ['CHORD']
TOL=1e-3
def load(case):
    ds=gdal.Open(f'tif/{case}.tif'); b=ds.GetRasterBand(1)
    gt=ds.GetGeoTransform()
    return b.ReadAsArray(), gt[1], (gt[0],gt[3]), b.GetNoDataValue()
def ring_eq(r1,r2):
    a=[tuple(p) for p in r1[:-1]]; b=[tuple(p) for p in r2[:-1]]
    if len(a)!=len(b): return False
    n=len(a)
    for s in range(n):
        if all(abs(a[i][0]-b[(i+s)%n][0])<=TOL and abs(a[i][1]-b[(i+s)%n][1])<=TOL for i in range(n)): return True
    return False
def vert_hits(r1,r2):
    # fraction of ArcGIS vertices that have a model vertex within TOL
    return sum(any(abs(p[0]-q[0])<=TOL and abs(p[1]-q[1])<=TOL for q in r1) for p in r2[:-1]), len(r2)-1
def score(case):
    arr,cs,org,nd=load(case)
    mine=simplify_polygons(arr,cs,org,None if nd is None else int(nd))
    ref=json.load(open(f'out/{case}.json'))
    used=set(); ok=0; vh=vt=0
    for f in ref:
        best=None
        for i,p in enumerate(mine):
            if i in used or p['value']!=f['gridcode'] or len(p['rings'])!=len(f['rings']): continue
            if all(any(ring_eq(mr,rr) for mr in p['rings']) for rr in f['rings']):
                best=i; break
        if best is not None: used.add(best); ok+=1
        for rr in f['rings']:
            allm=[q for p in mine if p['value']==f['gridcode'] for r in p['rings'] for q in r]
            h,t=vert_hits(allm,rr); vh+=h; vt+=t
    return ok,len(ref),len(mine),vh,vt
if __name__=='__main__':
    cases=sys.argv[1:] or sorted(os.path.basename(f)[:-5] for f in glob.glob('out/*.json') if os.path.exists('tif/'+os.path.basename(f)[:-5]+'.tif'))
    tot=[0]*5
    for c in cases:
        t=time.time(); r=score(c)
        tot=[x+y for x,y in zip(tot,r)]
        print(f'{"PASS" if r[0]==r[1]==r[2] else "FAIL"} {c:16s} polys {r[0]}/{r[1]} (model {r[2]})  vertices {r[3]}/{r[4]}  {time.time()-t:.1f}s',flush=True)
    print('TOTAL polys %d/%d vertices %d/%d'%(tot[0],tot[1],tot[3],tot[4]))
