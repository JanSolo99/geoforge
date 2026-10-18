import json, math, numpy as np
d=json.load(open('sc1.json'))
src=np.array(d['src']); R,C=src.shape; SENT=-123456789.0
srcs=[(r,c,src[r,c]) for r in range(R) for c in range(C) if src[r,c]!=-9999]
def out(k): return np.array(d[k]['values'],dtype=float)
# direction
def direction(maxd=None):
    o=np.full((R,C),SENT)
    for r in range(R):
        for c in range(C):
            best=None
            for (sr,sc,v) in srcs:
                dist=math.hypot(c-sc,r-sr)
                if best is None or dist<best[0]-1e-12: best=(dist,sr,sc,v)
            dist,sr,sc,v=best
            if maxd is not None and dist*30>maxd: continue
            if dist==0: o[r,c]=0; continue
            dx=(sc-c); dy=(r-sr)*-1   # to source: east positive, north = up = -row
            ang=math.degrees(math.atan2(dx,-(sr-r)*-1*-1)) if False else math.degrees(math.atan2(sc-c,r-sr))%360
            o[r,c]=ang
    return o
g=out('edir'); w=direction()
bad=[(r,c,g[r,c],w[r,c]) for r in range(R) for c in range(C) if abs(g[r,c]-w[r,c])>1e-3 and not(abs(g[r,c]-SENT)<10)]
print('edir bad',len(bad),bad[:5]); print(g[2,:6],w[2,:6]); print(d['edir']['pixel'], sorted(set(g.ravel()))[:10])

print('--- v2')
def comp(maxd=None,cs=30.0,mode='dir'):
    nc=int(round(C*30/cs)); nr=int(round(R*30/cs)); o=np.full((nr,nc),SENT)
    for r in range(nr):
        for c in range(nc):
            x=(c+0.5)*cs; y=nr*cs-(r+0.5)*cs   # from lower-left origin... top-left grid anchored lower-left
            y=(r+0.5)*cs    # distance from top edge going down
            best=None
            for (sr,sc,v) in srcs:
                sx=(sc+0.5)*30; sy=(sr+0.5)*30
                px=x; py=(nr*cs-(nr*cs-R*30)) if False else y
                dist=math.hypot(sx-x,sy-y)
                if best is None or dist<best[0]-1e-9: best=(dist,sx,sy,v)
            dist,sx,sy,v=best
            if maxd is not None and dist>maxd: continue
            if mode=='alloc': o[r,c]=v
            else: o[r,c]=0 if dist==0 else math.floor(math.degrees(math.atan2(sx-x,y-sy))%360+0.5)
    return o
for key,kw in [('edir',{}),('edir_md',{'maxd':200}),('edir_cs',{'cs':45.0})]:
    g=out(key); w=comp(mode='dir',**kw)
    if g.shape!=w.shape: print(key,'shape',g.shape,w.shape); continue
    bad=[(r,c,g[r,c],w[r,c]) for r in range(g.shape[0]) for c in range(g.shape[1]) if not(abs(g[r,c]-w[r,c])<1e-3 or (abs(g[r,c]-SENT)<10 and abs(w[r,c]-SENT)<10))]
    print(key,len(bad),bad[:4])
for key,kw in [('eall',{}),('eall_md',{'maxd':150})]:
    g=out(key); w=comp(mode='alloc',**kw)
    bad=[(r,c,g[r,c],w[r,c]) for r in range(R) for c in range(C) if not(abs(g[r,c]-w[r,c])<1e-3 or (abs(g[r,c]-SENT)<10 and abs(w[r,c]-SENT)<10))]
    print(key,len(bad),bad[:4])
print('edist nodata',d['edist']['nodata'], 'edir nodata',d['edir']['nodata'],'eall',d['eall']['pixel'],d['eall']['nodata'])

print('--- v3')
def comp2(maxd=None,cs=30.0,mode='dir'):
    nc=int(round(C*30/cs)); nr=int(round(R*30/cs)); o=np.full((nr,nc),SENT)
    s2=[]
    for r in range(nr):
        for c in range(nc):
            x=(c+0.5)*cs; y=(r+0.5)*cs
            sc=int(math.floor(x/30)); sr=int(math.floor(y/30))
            if 0<=sr<R and 0<=sc<C and src[sr,sc]!=-9999: s2.append((x,y,src[sr,sc]))
    for r in range(nr):
        for c in range(nc):
            x=(c+0.5)*cs; y=(r+0.5)*cs
            best=None
            for (sx,sy,v) in s2:
                dist=math.hypot(sx-x,sy-y)
                if best is None or dist<best[0]-1e-9: best=(dist,sx,sy,v)
            dist,sx,sy,v=best
            if maxd is not None and dist>maxd: continue
            if mode=='alloc': o[r,c]=v
            else:
                if dist==0: o[r,c]=0
                else:
                    a=math.degrees(math.atan2(sx-x,y-sy))%360; a=math.floor(a+0.5); o[r,c]=360 if a==0 else a
    return o
for key,kw in [('edir',{}),('edir_md',{'maxd':200}),('edir_cs',{'cs':45.0})]:
    g=out(key); w=comp2(mode='dir',**kw)
    bad=[(r,c,g[r,c],w[r,c]) for r in range(g.shape[0]) for c in range(g.shape[1]) if not(abs(g[r,c]-w[r,c])<1e-3 or (abs(g[r,c]-SENT)<10 and abs(w[r,c]-SENT)<10))]
    print(key,len(bad),bad[:4])
for key,kw in [('eall',{}),('eall_md',{'maxd':150})]:
    g=out(key); w=comp2(mode='alloc',**kw)
    bad=[(r,c,g[r,c],w[r,c]) for r in range(R) for c in range(C) if not(abs(g[r,c]-w[r,c])<1e-3 or (abs(g[r,c]-SENT)<10 and abs(w[r,c]-SENT)<10))]
    print(key,len(bad),bad[:4])
