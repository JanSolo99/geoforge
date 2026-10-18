import json, random
random.seed(7); c={}
def rnd(H,W,n):
    s=set()
    while len(s)<n: s.add((random.randrange(H),random.randrange(W)))
    return sorted(s)
c['big1']=[120,150,rnd(120,150,400)]
c['big2']=[80,80,rnd(80,80,640)]
c['big3']=[150,120,rnd(150,120,60)]
# lines
s=set()
for k in range(6):
    r,cc=random.randrange(100),random.randrange(100); dr,dc=random.choice([(0,1),(1,0),(1,1),(1,-1),(1,2),(2,1)])
    for t in range(40):
        rr,c2=r+dr*t//2,cc+dc*t//2
        if 0<=rr<100 and 0<=c2<100: s.add((rr,c2))
c['lines']=[100,100,sorted(s)]
json.dump(c,open('ex2/cfg.json','w'))
