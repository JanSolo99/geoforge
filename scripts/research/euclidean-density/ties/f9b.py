import pickle, sim2
from f9 import build
from multiprocessing import Pool
for d in pickle.load(open('ds2.pkl','rb')): sim2.D[d[0]]=d
names=['streams','big1','big2','big3','lines']
def ev(a):
    sw=build(a)
    return (sum(sim2.score(n,sw) for n in names), sum(sim2.wrong_dist(n,sw) for n in names), [sim2.score(n,sw) for n in names], a)
if __name__=='__main__':
    r=[a for b,a in pickle.load(open('f9.pkl','rb')) if b==0]
    with Pool(12) as p: res=p.map(ev,r)
    res.sort(key=lambda x:(x[0]+x[1],repr(x)))
    for x in res: print(x)
