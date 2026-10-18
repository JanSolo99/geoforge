from sim2 import *
from multiprocessing import Pool
import numpy as np
order=['p_0_2','p_0_4','p_1_-3','p_2_-4','p_3_-1','p_4_-2','sq4','plus','p_1_1','p_1_-1','p_2_0','rnd0','rnd3','rnd2','rnd1']
E=(0,1); Wn=(0,-1)
def vpass(rowdir, perm, inrow_pos, coldir, strict, second):
    pr=-1 if rowdir=='d' else 1
    base=[(pr,-1),(pr,0),(pr,1)]
    m=[base[i] for i in perm]
    prev=(0,-1) if coldir=='r' else (0,1)
    if inrow_pos is not None: m.insert(inrow_pos, prev)
    sw=[(coldir,m,strict)]
    if second is not None:
        oc='l' if coldir=='r' else 'r'
        sw.append((oc,[(0,-1) if oc=='r' else (0,1)],second))
    return (rowdir,sw)
H0=('d',[('l',[E],True),('r',[Wn],True)])
def build(a):
    d,u=a
    return [H0,vpass('d',*d),vpass('u',*u)]
TR={}
def wd_small(name,p):
    return wrong_dist(name,p)
def ev(a):
    sw=build(a); b=0
    for n in order:
        b+=score(n,sw)
        if b>0: return (b,a)
    for n in ['rnd0','rnd2','rnd3','rnd1']:
        b+=wrong_dist(n,sw)
    return (b,a)
P3=list(itertools.permutations(range(3)))
def opts(rowdir):
    return [(p,ip,cd,st,sec) for p in P3 for ip in [None,0,1,2,3] for cd in 'rl' for st in [True,False] for sec in [None,True,False]
            if not (ip is None and cd=='l')]
if __name__=='__main__':
    args=[(d,u) for d in opts('d') for u in opts('u')]
    print(len(args),flush=True)
    with Pool(12) as p: res=p.map(ev,args,chunksize=100)
    pickle.dump(res,open("f9.pkl","wb")); res.sort(key=lambda x:(x[0],repr(x[1]))); [print(x) for x in res[:30]]
    pickle.dump(res,open('f9.pkl','wb'))
