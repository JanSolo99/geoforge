import csv, math, json, subprocess, itertools
L='ABCDEFGHJKLMNPQRSTUVWXYZ'
def decode(s,fmt):
 s=''.join(s.upper().split())
 try:
  if fmt=='GARS':
   x=(int(s[:3])-1)/2-180;y=(L.index(s[3])*24+L.index(s[4]))/2-90
   if len(s)>5:
    q=int(s[5]);x+=.25*(q in (2,4));y+=.25*(q in (1,2))
   if len(s)>6:
    k=int(s[6])-1;x+=(k%3)/12;y+=(2-k//3)/12
  else:
   n=(len(s)-4)//2;scale=10**(n-2)*60
   x=L.index(s[0])*15+L.index(s[2])-180+int(s[4:4+n])/scale
   y=L.index(s[1])*15+L.index(s[3])-90+int(s[4+n:])/scale
  return x,y
 except: return None

def cells(s):
 z=int(s[:3])-1;r=L.index(s[3])*24+L.index(s[4]);q=int(s[5]);k=int(s[6])-1
 return z*6+(3 if q in(2,4) else 0)+k%3,r*6+(3 if q in(1,2) else 0)+(2-k//3)

def file(p):
 return json.loads(subprocess.check_output(['ogrinfo','-ro','-al','-json','-features',p],stderr=subprocess.DEVNULL))['layers'][0]['features']
rows=[]
for fmt in ['GARS','GEOREF']:
 raw=list(csv.DictReader(open(f'testdata/geoproc/grid_input_{fmt.lower()}.csv')))
 ref=file(f'scripts/research/coordinate_notation/grid/unsupported_gars_outputs/ccn_grid_input_{fmt.lower()}_gars.shp')
 for a,b in zip(raw,ref):
  if b['properties']['GARS']:
   rows.append((fmt,a['ID'],decode(a['COORD'],fmt),cells(b['properties']['GARS'])))
line=open('scripts/research/coordinate_notation/grid/probe_boundaries_results.txt').readline()
for i,(raw,expected) in enumerate(json.loads(line.split(' ',1)[1])):
 rows.append(('holdout',str(i),decode(raw,'GARS'),cells(expected)))
# candidate tick positions, direct arithmetic or radians scale
funcs={
 'deg': lambda v,o:(v+o)*12,
 'deg2':lambda v,o: v*12+o*12,
 'rad':lambda v,o:(math.radians(v)+math.radians(o))/(math.pi/2160),
 'rad2':lambda v,o:(math.radians(v)+math.radians(o))*2160/math.pi,
 'rad3':lambda v,o:(v*(math.pi/180)+o*(math.pi/180))*(2160/math.pi),
 'raddeg':lambda v,o:(math.degrees(math.radians(v))+o)*12,
 'raddeg2':lambda v,o:(math.radians(v)*(180/math.pi)+o)*12,
 'degmin': lambda v,o:((v+o)*60)/5,
 'degmin2':lambda v,o:(v*60+o*60)/5,
}
for axis,o in [(0,180),(1,90)]:
 rank=[]
 for name,fn in funcs.items():
  bad=[]
  for fmt,i,xy,w in rows:
   got=math.floor(fn(xy[axis],o));got=got%4320 if axis==0 else min(got,2159)
   if got!=w[axis]:bad.append((fmt,i,got,w[axis],xy[axis]))
  rank.append((len(bad),name,bad[:4]))
 print(axis,sorted(rank))
for axis,o in [(0,180),(1,90)]:
 for trans in [lambda v:v,lambda v:math.degrees(math.radians(v)),lambda v:math.radians(v)*(180/math.pi),lambda v:(v*math.pi/180)*180/math.pi]:
  for op in [lambda v,o:(v+o)*12,lambda v,o:v*12+o*12, lambda v,o:(v*60+o*60)/5]:
   bad=[]
   for fmt,i,xy,w in rows:
    got=math.floor(op(trans(xy[axis]),o));got=got%4320 if axis==0 else min(got,2159)
    if got!=w[axis]:bad.append((fmt,i,got,w[axis]))
   print('combo',axis,trans.__code__.co_firstlineno,op.__code__.co_firstlineno,len(bad),bad[:3])
