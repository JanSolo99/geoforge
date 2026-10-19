import json,math,sys
def split(p):
    rings=[];cur=[]
    for q in p:
        if q is None: rings.append(cur);cur=[]
        else: cur.append(tuple(q))
    rings.append(cur); return [r[:-1] for r in rings]
def top_right(r): return max(range(len(r)),key=lambda i:(r[i][1],r[i][0]))
def lowest(r): return min(range(len(r)),key=lambda i:(r[i][1],r[i][0]))
def canon_ccw(h):
    # rotate hole to start at lowest (canonical) keep orientation
    b=lowest(h); return h[b:]+h[:b]
def diagonals(rings, variant):
    at=lambda v: rings[v[0]][v[1]]
    L=lambda r: len(rings[r])
    prev=lambda v:(v[0],(v[1]-1)%L(v[0])); nxt=lambda v:(v[0],(v[1]+1)%L(v[0]))
    before=lambda a,b: (a[1],a[0])<(b[1],b[0])
    ev=sorted([(r,i) for r in range(len(rings)) for i in range(L(r))],key=lambda v:(at(v)[1],at(v)[0]))
    def kind(v):
        a,b,c=at(prev(v)),at(v),at(nxt(v))
        turn=(b[0]-a[0])*(c[1]-b[1])-(b[1]-a[1])*(c[0]-b[0])
        pa,nb=before(b,a),before(b,c)
        if pa and nb: return 'start' if turn<0 else 'split'
        if not pa and not nb: return 'end' if turn<0 else 'merge'
        return 'reg'
    active=[]; diags=[]
    def xat(e,y):
        a,b=at(e),at(nxt(e))
        if a[1]==b[1]: return max(a[0],b[0])
        return a[0]+(y-a[1])*(b[0]-a[0])/(b[1]-a[1])
    def left(v):
        p=at(v); best=None
        for i,(e,h) in enumerate(active):
            if e==v or nxt(e)==v: continue
            x=xat(e,p[1])
            if x<=p[0] and (best is None or x>best[1]): best=(i,x)
        return None if best is None else best[0]
    def take_prev(v):
        pv=prev(v)
        for i,(e,h) in enumerate(active):
            if e==pv:
                if kind(h)=='merge': diags.append((v,h))
                active.pop(i); return
    for v in ev:
        k=kind(v)
        if k=='start': active.append([v,v])
        elif k=='end': take_prev(v)
        elif k=='split':
            j=left(v)
            if j is not None: diags.append((v,active[j][1])); active[j][1]=v
            active.append([v,v])
        elif k=='merge':
            take_prev(v); j=left(v)
            if j is not None:
                if kind(active[j][1])=='merge': diags.append((v,active[j][1]))
                active[j][1]=v
        else:
            if before(at(prev(v)),at(v)):
                take_prev(v); active.append([v,v])
            else:
                j=left(v)
                if j is not None:
                    if kind(active[j][1])=='merge': diags.append((v,active[j][1]))
                    active[j][1]=v
    return diags
def walk(rings, diags, variant):
    at=lambda v: rings[v[0]][v[1]]
    L=lambda r: len(rings[r])
    links={}
    for a,b in diags:
        if a[0]!=b[0]:
            links.setdefault(a,[]).append(b); links.setdefault(b,[]).append(a)
    reached=[False]*len(rings); reached[0]=True; order=[]
    def visit(r, entry):
        n=L(r)
        steps = list(range(1,n+1))
        if variant.get('entry_first'): steps=[0]+list(range(1,n))
        for st in steps:
            v=(r,(entry+st)%n)
            while True:
                ts=links.get(v,[])
                here=at(v); frm=at((r,(v[1]-1)%n))
                back=math.atan2(frm[1]-here[1],frm[0]-here[0])
                ts=sorted(ts,key=lambda t:(math.atan2(at(t)[1]-here[1],at(t)[0]-here[0])-back)%(2*math.pi))
                t=next((t for t in ts if not reached[t[0]]),None)
                if t is None: break
                reached[t[0]]=True; order.append(t); visit(t[0],t[1])
    visit(0, top_right(rings[0]))
    return order
def run(f,key,variant={}):
    d=json.load(open(f))[key]
    bad=tot=0
    for a,parts in d['out']['rows']:
        rs=split(parts[0])
        if len(rs)<2: continue
        outer=rs[0]; holes_arc=rs[1:]
        # canonical inputs: holes as sets rotated to lowest; order shuffled by sorting
        holes=sorted([canon_ccw(h) for h in holes_arc])
        rings=[outer]+holes
        ds=diagonals(rings,variant)
        order=walk(rings,ds,variant)
        out=[]
        for r,l in order:
            h=rings[r]; k=len(h); out.append([h[(l+1+i)%k] for i in range(k)])
        tot+=1
        if out!=holes_arc:
            bad+=1
            if variant.get('show'):
                for i,(x,y) in enumerate(zip(out,holes_arc)):
                    print(' ', i, 'OK' if x==y else 'ours %s arc %s'%(x[:2],y[:2]))
    print(key,'faces with holes',tot,'bad',bad)
    return bad
if __name__=='__main__':
    v={'show':True} if 'show' in sys.argv else {}
    for f,k in [('pg/p4.json','ftp_vp'),('pg/p4.json','ftp_t4'),('pg/p4.json','ftp_t3'),('pg/p12.json','hh_ftp'),('pg/p13.json','tri_ftp'),('pg/p14.json','f2_ftp')]:
        run(f,k,v)
