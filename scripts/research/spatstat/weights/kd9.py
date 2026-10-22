import json
from msvc import nth_element
o7=json.load(open('sw2/out7.json'))
def order(X, ls, isort=32, leafsort='none'):
    a=list(range(len(X)))
    def rec(f, l, depth):
        ids=a[f:l]
        sx=max(X[i][0] for i in ids)-min(X[i][0] for i in ids); sy=max(X[i][1] for i in ids)-min(X[i][1] for i in ids)
        dm=0 if sx>=sy else 1
        n=l-f; m=f+n//2
        if n > ls or depth==0:
            nth_element(a, f, m, l, lambda p,q: X[p][dm]<X[q][dm], isort)
        if n > ls:
            rec(f, m, depth+1); rec(m, l, depth+1)
        elif leafsort=='sort' and depth>0:
            a[f:l]=sorted(a[f:l], key=lambda p:X[p][dm])
    rec(0, len(a), 0)
    return a
if __name__=='__main__':
    good=None
    for name,v in o7['kd'].items():
        X=[tuple(p) for p in v['pts']]
        ok=set()
        for ls in range(1,20):
            for isort in [16,32]:
                for lsrt in ["none"]:
                    o=order(X,ls,isort,lsrt); rank={j:k for k,j in enumerate(o)}
                    if all(sorted([u-1 for u in r[1]],key=lambda j:rank[j])==[u-1 for u in r[1]] for r in v['swm']): ok.add((ls,isort,lsrt))
        good=ok if good is None else good&ok
        print(name, len(ok), sorted(ok)[:4])
    print(sorted(good))
# check other datasets
from kd import get, d, cases
for ls in range(1,20):
    tot=0
    for name,case in cases:
        X,U=get(name); o=order(X,ls); rank={j:k for k,j in enumerate(o)}; ixm={u:k for k,u in enumerate(U)}
        for r in d['swm'][case]['rows']:
            l=[ixm[u] for u in r[1]]; tot+= sorted(l,key=lambda j:rank[j])==l
    print(ls, tot)
