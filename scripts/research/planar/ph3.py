from ph import split, top_right, lowest, canon_ccw, diagonals
import json, math, itertools
def walkv(rings, diags, ringdir, entryfirst, multi, angdir, outerdir):
    at=lambda v: rings[v[0]][v[1]]
    L=lambda r: len(rings[r])
    links={}
    for a,b in diags:
        if a[0]!=b[0]:
            links.setdefault(a,[]).append(b); links.setdefault(b,[]).append(a)
    reached=[False]*len(rings); reached[0]=True; order=[]
    def visit(r, entry, dirn):
        n=L(r)
        if dirn==1: seq=[(entry+k)%n for k in range(1,n+1)]
        else: seq=[(entry-k)%n for k in range(1,n+1)]
        if entryfirst: seq=[seq[-1]]+seq[:-1]
        for i in seq:
            v=(r,i)
            here=at(v); frm=at((r,(i-dirn)%n))
            back=math.atan2(frm[1]-here[1],frm[0]-here[0])
            ts=sorted(links.get(v,[]),key=lambda t:(angdir*(math.atan2(at(t)[1]-here[1],at(t)[0]-here[0])-back))%(2*math.pi))
            for t in ts:
                if not reached[t[0]]:
                    reached[t[0]]=True; order.append(t); visit(t[0],t[1],ringdir)
                    if not multi: break
    visit(0, top_right(rings[0]), outerdir)
    return order
def score(variant, cases):
    bad=0; tot=0
    for f,k in cases:
        d=json.load(open(f))[k]
        for a,parts in d['out']['rows']:
            rs=split(parts[0])
            if len(rs)<2: continue
            outer=rs[0]; ha=rs[1:]
            holes=sorted([canon_ccw(h) for h in ha]); rings=[outer]+holes
            ds=diagonals(rings,{})
            order=walkv(rings,ds,*variant)
            out=[]
            for r,l in order:
                h=rings[r]; kk=len(h); out.append([h[(l+1+i)%kk] for i in range(kk)])
            tot+=len(ha); bad+=sum(1 for x,y in itertools.zip_longest(out,ha) if x!=y)
    return bad,tot
cases=[('pg/p4.json','ftp_vp'),('pg/p4.json','ftp_t4'),('pg/p4.json','ftp_t3'),('pg/p12.json','hh_ftp'),('pg/p13.json','tri_ftp'),('pg/p14.json','f2_ftp')]
res=[]
for v in itertools.product([1,-1],[False,True],[True,False],[1,-1],[1]):
    res.append((score(v,cases),v))
for r in sorted(res): print(r)
