import pickle, itertools
D={d[0]:d for d in pickle.load(open('ds.pkl','rb'))}
INF=1<<60
def simulate(H,W,cells,passes):
    # passes: list of (rowdir, [(coldir, nbs, strict), ...]) ; sweeps interleaved per row
    best=[-1]*(H*W); bd=[INF]*(H*W)
    for i,(r,c) in enumerate(cells):
        best[r*W+c]=i; bd[r*W+c]=0
    for ro,sweeps in passes:
        rows=range(H) if ro=='d' else range(H-1,-1,-1)
        for r in rows:
            for co,nbs,strict in sweeps:
                cols=range(W) if co=='r' else range(W-1,-1,-1)
                for c in cols:
                    k=r*W+c; cb=bd[k]; cs=best[k]
                    for dr,dc in nbs:
                        rr=r+dr; cc=c+dc
                        if 0<=rr<H and 0<=cc<W:
                            s=best[rr*W+cc]
                            if s<0: continue
                            sr,sc=cells[s]; d=(sr-r)**2+(sc-c)**2
                            if d<cb or (not strict and d==cb):
                                cb=d; cs=s
                    bd[k]=cb; best[k]=cs
    return best,bd
def score(name,p):
    _,H,W,cells,truth=D[name]
    best,bd=simulate(H,W,cells,p)
    return sum(1 for (r,c),t in truth.items() if best[r*W+c]!=t)
def wrong_dist(name,p):
    _,H,W,cells,truth=D[name]
    best,bd=simulate(H,W,cells,p)
    import numpy as np
    S=np.array(cells); n=0
    for r in range(H):
        for c in range(W):
            if bd[r*W+c]!=((S[:,0]-r)**2+(S[:,1]-c)**2).min(): n+=1
    return n
def nb(rowdir,coldir):
    pr=-1 if rowdir=='d' else 1
    pc=-1 if coldir=='r' else 1
    return [(pr,-1),(pr,0),(pr,1),(0,pc)]
def build(a):
    r1,c1,c2,m1,m2,st=a
    r2='u' if r1=='d' else 'd'
    n1=nb(r1,c1); n2=nb(r2,c2)
    o1='l' if c1=='r' else 'r'; o2='l' if c2=='r' else 'r'
    return [(r1,[(c1,[n1[i] for i in m1],st[0]),(o1,[(0,-1 if o1=='r' else 1)],st[1])]),
            (r2,[(c2,[n2[i] for i in m2],st[2]),(o2,[(0,-1 if o2=='r' else 1)],st[3])])]
