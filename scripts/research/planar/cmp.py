import json,sys,subprocess,os,glob
GF='/mnt/media/ArcGIS_Linux/target/debug/ug'
X0,Y0=500000,5000000
PG='pg'
def gf_out(args, out):
    for e in ('shp','shx','dbf','prj','cpg'):
        try: os.remove(out[:-3]+e)
        except FileNotFoundError: pass
    r=subprocess.run([GF]+args,capture_output=True,text=True)
    if r.returncode: return None, r.stderr
    js=out[:-4]+'.geojson'
    if os.path.exists(js): os.remove(js)
    subprocess.run(['ogr2ogr','-f','GeoJSON','-lco','COORDINATE_PRECISION=7',js,out],check=True,capture_output=True)
    return json.load(open(js)), None
def rows_of(gj):
    rows=[]
    for f in gj['features']:
        g=f['geometry']; props=list(f['properties'].values())
        if g is None: rows.append((props,[])); continue
        if g['type']=='LineString': parts=[g['coordinates']]
        elif g['type']=='MultiLineString': parts=g['coordinates']
        elif g['type']=='Polygon': parts=g['coordinates']
        elif g['type']=='MultiPolygon': parts=[r for p in g['coordinates'] for r in p]
        rows.append((props,[[[x-X0,y-Y0] for x,y in p] for p in parts]))
    return rows
def arc_rows(out):
    rows=[]
    for a,parts in out['rows']:
        rr=[]
        for p in parts:
            cur=[]
            for q in p:
                if q is None: rr.append(cur);cur=[]
                else: cur.append(q)
            rr.append(cur)
        rows.append((a[1:],rr))
    return rows
def same_val(x,y):
    if x is None: x=0
    if y is None: y=0
    if isinstance(x,str) or isinstance(y,str): return str(x).strip()==str(y).strip()
    return abs(x-y)<1e-9
def compare(case, ours, arc, tol=0.002):
    diffs=[]
    if len(ours)!=len(arc): diffs.append('count ours %d arc %d'%(len(ours),len(arc)))
    for i,((pa,ga),(pb,gb)) in enumerate(zip(ours,arc)):
        if len(pa)!=len(pb) or not all(same_val(x,y) for x,y in zip(pa,pb)): diffs.append('row %d attrs ours %s arc %s'%(i,pa,pb))
        # ArcGIS rings vs ours: compare part by part
        if len(ga)!=len(gb): diffs.append('row %d parts ours %d arc %d'%(i,len(ga),len(gb))); continue
        for k,(p,q) in enumerate(zip(ga,gb)):
            if len(p)!=len(q) or any(abs(a[0]-b[0])>tol or abs(a[1]-b[1])>tol for a,b in zip(p,q)):
                diffs.append('row %d part %d ours %s\n        arc  %s'%(i,k,[[round(c,3) for c in v] for v in p],[[round(c,3) for c in v] for v in q])); 
    return diffs
if __name__=='__main__':
    tool=sys.argv[1]; jf=sys.argv[2]; keys=sys.argv[3:]
    d=json.load(open(jf))
    for k in keys:
        inp=PG+'/'+sys.argv[3+keys.index(k)] if False else None
