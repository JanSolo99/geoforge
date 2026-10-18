import json, random
c={}
N=31; m=15
for dr in range(0,5):
    for dc in range(-4,5):
        if (dr,dc)<=(0,0): continue
        c['p_%d_%d'%(dr,dc)]=[N,N,[(m,m),(m+dr,m+dc)]]
c['sq4']=[N,N,[(m,m),(m,m+2),(m+2,m),(m+2,m+2)]]
c['plus']=[N,N,[(m-2,m),(m+2,m),(m,m-2),(m,m+2)]]
random.seed(1)
for k in range(4):
    cells=set()
    while len(cells)<25: cells.add((random.randrange(60),random.randrange(60)))
    c['rnd%d'%k]=[60,60,sorted(cells)]
json.dump(c,open('ex/cfg.json','w')); print(len(c))
