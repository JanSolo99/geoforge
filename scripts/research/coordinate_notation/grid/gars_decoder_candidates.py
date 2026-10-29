import json,math
L='ABCDEFGHJKLMNPQRSTUVWXYZ'
p='scripts/research/coordinate_notation/grid/probe_boundaries_results.txt'
rows=json.loads(open(p).readline().split(' ',1)[1])
def tick(s,axis):
 q,k=int(s[5]),int(s[6])-1
 return (int(s[:3])-1)*6+3*(q in(2,4))+k%3 if axis==0 else (L.index(s[3])*24+L.index(s[4]))*6+3*(q in(1,2))+(2-k//3)
for axis,offset in [(0,180),(1,90)]:
 funcs={
 'naive':lambda b,q,k,o:b/2-o+q/4+k/12,
 'offset_last':lambda b,q,k,o:b/2+q/4+k/12-o,
 'rational':lambda b,q,k,o:(b*6+q*3+k-o*12)/12,
 'rational_offset':lambda b,q,k,o:(b*6+q*3+k)/12-o,
 'offset_inner':lambda b,q,k,o:b/2-o+(q*3+k)/12,
 'ticks_mul':lambda b,q,k,o:(b*6+q*3+k)*(1/12)-o,
 'rad_naive':lambda b,q,k,o:math.degrees(math.radians(b/2-o)+math.radians(q/4)+math.radians(k/12)),
 'rad_offsetlast':lambda b,q,k,o:math.degrees(math.radians(b/2)+math.radians(q/4)+math.radians(k/12)-math.radians(o)),
 'rad_ticks':lambda b,q,k,o:math.degrees((b*6+q*3+k-o*12)*(math.pi/2160)),
 'rad_ticksdiv':lambda b,q,k,o:math.degrees((b*6+q*3+k-o*12)*math.pi/2160),
 'rad_offsetticks':lambda b,q,k,o:math.degrees((b*6+q*3+k)*(math.pi/2160)-math.radians(o)),
 }
 rank=[]
 for name,fn in funcs.items():
  for roundtrip in [False,True]:
   bad=[]
   for raw,expected in rows:
    q,k=int(raw[5]),int(raw[6])-1
    b=int(raw[:3])-1 if axis==0 else L.index(raw[3])*24+L.index(raw[4])
    qe=int(q in ((2,4) if axis==0 else (1,2)));ke=k%3 if axis==0 else 2-k//3
    value=fn(b,qe,ke,offset)
    if roundtrip:value=math.degrees(math.radians(value))
    got=math.floor((value+offset)*12);got=got%4320 if axis==0 else min(got,2159)
    if got!=tick(expected,axis):bad.append((raw,expected,value,got,tick(expected,axis)))
   rank.append((len(bad),name,roundtrip,bad[:3]))
 print(axis,*sorted(rank)[:15],sep='\n')
