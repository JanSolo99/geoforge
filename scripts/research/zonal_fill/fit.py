import json, sys, itertools
cases=json.load(open(sys.argv[1]))
N4=[(-1,0),(1,0),(0,-1),(0,1)]
N8=N4+[(-1,-1),(-1,1),(1,-1),(1,1)]
def zones(z): return sorted({v for r in z for v in r})
def model(z,w,nb,edge,kind):
    h,W=len(z),len(z[0]); res={}
    for zz in zones(z):
        vals=[]
        for i in range(h):
            for j in range(W):
                if z[i][j]!=zz: continue
                for di,dj in nb:
                    a,b=i+di,j+dj
                    if 0<=a<h and 0<=b<W:
                        if z[a][b]!=zz:
                            if kind=='max': vals.append(max(w[i][j],w[a][b]))
                            elif kind=='in': vals.append(w[i][j])
                            elif kind=='out': vals.append(w[a][b])
                    elif edge:
                        vals.append(w[i][j])
        res[zz]=min(vals) if vals else min(w[i][j] for i in range(h) for j in range(W) if z[i][j]==zz)
    return [[res[z[i][j]] for j in range(W)] for i in range(h)]
for nb,nbn in ((N4,'4'),(N8,'8')):
    for edge in (False,True):
        for kind in ('max','in','out'):
            ok=sum(model(c['z'],c['w'],nb,edge,kind)==c['out'] for c in cases)
            print(nbn,edge,kind,ok,'/',len(cases))

def flood(z,w,nb,edge_in=True):
    h,W=len(z),len(z[0]); zs=zones(z)
    INF=float('inf'); level={zz:INF for zz in zs}
    # edge spill
    for i in range(h):
        for j in range(W):
            if edge_in and (i in (0,h-1) or j in (0,W-1)):
                level[z[i][j]]=min(level[z[i][j]], w[i][j])
    changed=True
    while changed:
        changed=False
        for i in range(h):
            for j in range(W):
                for di,dj in nb:
                    a,b=i+di,j+dj
                    if 0<=a<h and 0<=b<W and z[a][b]!=z[i][j]:
                        v=max(w[i][j],w[a][b],level[z[a][b]])
                        if v<level[z[i][j]]:
                            level[z[i][j]]=v; changed=True
    return [[level[z[i][j]] for j in range(W)] for i in range(h)]
for nb,nbn in ((N4,'4'),(N8,'8')):
    ok=sum(flood(c['z'],c['w'],nb)==c['out'] for c in cases)
    print('flood',nbn,ok,'/',len(cases))

shown=0
for c in cases:
    m=model(c['z'],c['w'],N8,True,'max')
    if m!=c['out'] and shown<3:
        shown+=1
        print('z',c['z']); print('w',[[int(v) for v in r] for r in c['w']])
        zz={}
        for i,r in enumerate(c['z']):
            for j,v in enumerate(r): zz.setdefault(v,set()).add((c['out'][i][j], m[i][j]))
        print('zone: (arcgis, model)', zz)

def model2(z,w,nb,sides):
    h,W=len(z),len(z[0]); res={}
    for zz in zones(z):
        vals=[]
        for i in range(h):
            for j in range(W):
                if z[i][j]!=zz: continue
                for di,dj in nb:
                    a,b=i+di,j+dj
                    if 0<=a<h and 0<=b<W:
                        if z[a][b]!=zz: vals.append(max(w[i][j],w[a][b]))
                    else:
                        side=('T' if a<0 else 'B' if a>=h else '')+('L' if b<0 else 'R' if b>=W else '')
                        if any(s in sides for s in side): vals.append(w[i][j])
        res[zz]=min(vals) if vals else min(w[i][j] for i in range(h) for j in range(W) if z[i][j]==zz)
    return [[res[z[i][j]] for j in range(W)] for i in range(h)]
import itertools
for nb,nbn in ((N4,'4'),(N8,'8')):
    for r in range(0,5):
        for sides in itertools.combinations('TBLR',r):
            ok=sum(model2(c['z'],c['w'],nb,set(sides))==c['out'] for c in cases)
            if ok>=15: print('sides',nbn,''.join(sides),ok)
