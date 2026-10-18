# datasets: list of (name, H, W, sources list[(r,c)], truth dict {(r,c): src index} for tie cells)
import json, numpy as np, pickle
def load_all():
    out=[]
    cfg=json.load(open('ex/cfg.json')); o=json.load(open('ex/out.json'))
    for name,(H,W,cells) in cfg.items():
        A=np.array(o[name]['alloc']); S=np.array(cells)
        truth={}
        for r in range(H):
            for c in range(W):
                d2=(S[:,0]-r)**2+(S[:,1]-c)**2; m=d2.min(); idx=np.where(d2==m)[0]
                if len(idx)>1: truth[(r,c)]=int(A[r,c])-1
        out.append((name,H,W,[tuple(x) for x in cells],truth))
    from load import src, dirr, ang
    H,W=src.shape; cells=[(r,c) for r in range(H) for c in range(W) if src[r,c]==1]
    info=pickle.load(open('info.pkl','rb')); idx={s:i for i,s in enumerate(cells)}
    truth={(r,c):idx[(int(ok[0][0]),int(ok[0][1]))] for r,c,m,cs,ok in info}
    out.append(('streams',H,W,cells,truth))
    return out
if __name__=='__main__':
    D=load_all(); pickle.dump(D,open('ds.pkl','wb'))
    for d in D: print(d[0],len(d[4]))
