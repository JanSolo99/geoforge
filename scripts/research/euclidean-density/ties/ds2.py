import json, numpy as np, pickle
out=[]
cfg=json.load(open('ex2/cfg.json')); o=json.load(open('ex2/out.json'))
nd=0
for name,(H,W,cells) in cfg.items():
    A=np.array(o[name]['alloc']); S=np.array(cells); truth={}
    for r in range(H):
        for c in range(W):
            d2=(S[:,0]-r)**2+(S[:,1]-c)**2; m=d2.min(); idx=np.where(d2==m)[0]
            if A[r,c]-1 not in idx: nd+=1
            if len(idx)>1: truth[(r,c)]=int(A[r,c])-1
    out.append((name,H,W,[tuple(map(int,x)) for x in cells],truth)); print(name,len(truth))
print('not nearest',nd)
pickle.dump(out,open('ds2.pkl','wb'))
