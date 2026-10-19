import json,sys
from cmp import *
C=[]
def p2l(f,k,inp): C.append((f,k,['polygon-to-line',PG+'/'+inp+'.shp','OUT']))
def ftl(f,k,inps,*extra): C.append((f,k,['feature-to-line']+[PG+'/'+i+'.shp' for i in inps]+['OUT']+list(extra)))
def ftp(f,k,inps,*extra): C.append((f,k,['feature-to-polygon']+[PG+'/'+i+'.shp' for i in inps]+['OUT']+list(extra)))
p2l('p1','p2l_grid','grid'); p2l('p1','p2l_vp','vp')
for t in ['t2','t1','t3','t4','t6']: p2l('p2',t,t)
for t in ['t7','t8','t9','t10']: p2l('p3',t,t)
p2l('p7','d1_p2l','d1')
for c in ['c1','c2','c3','c4','c5','c6']: p2l('p10',c+'_p2l',c)
p2l('p14','f2_p2l','f2')
ftl('p4','ftl_vp',['vp']); ftl('p4','ftl_t4',['t4']); ftl('p6','ftl_grid',['grid']); ftl('p7','d1_ftl',['d1'])
for i in range(4): ftl('p7','rs%d_ftl'%i,['rs%d'%i])
ftl('p11','b1_ftl',['b1']); ftl('p14','f1_ftl',['f1']); ftl('p14','f1_ftl_na',['f1'],'--attributes','NO_ATTRIBUTES'); ftl('p14','f2_ftl',['f2'])
ftl('p15','ftl_multi',['f1','f2']); ftl('p15','ftl_multi_na',['f1','f2'],'--attributes','NO_ATTRIBUTES')
ftl('p5','ftl_L1',['L1'])
for k,i in [('ftp_vp','vp'),('ftp_t4','t4'),('ftp_t10','t10'),('ftp_t3','t3')]: ftp('p4',k,[i])
ftp('p5','ftp_L1',['L1']); ftp('p6','ftp_grid',['grid']); ftp('p7','d1_ftp',['d1'])
for c in ['c1','c2','c3','c4','c5','c6']: ftp('p10',c+'_ftp',[c])
ftp('p11','b1_ftp',['b1']); ftp('p11','b1_ftpl',['b1'],'--label-features',PG+'/lb.shp')
ftp('p12','hh_ftp',['hh']); ftp('p13','tri_ftp',['tri']); ftp('p14','f2_ftp',['f2']); ftp('p15','ftp_multi_ab',['f1','f2'])
ftp('p8','ftp_na_L1',['L1'],'--attributes','NO_ATTRIBUTES')
sel=sys.argv[1:]
for f,k,args in C:
    if sel and not any(s in k for s in sel): continue
    arc=json.load(open(PG+'/'+f+'.json'))[k]['out']
    out=PG+'/gf_'+k+'.shp'
    args=[out if a=='OUT' else a for a in args]
    gj,e=gf_out(args,out)
    if gj is None: print('FAIL',k,e[:300]); continue
    d=compare(k,rows_of(gj),arc_rows(arc))
    print(('OK  ' if not d else 'DIFF'),k,len(d))
    for x in d[:4]: print('    ',x)
