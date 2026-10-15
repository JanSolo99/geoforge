import numpy as np
CASES={}
def add(name,a,cs=1): CASES[name]=(np.array(a),cs)
def stair(n,run=1,rise=1,size=None):
    # lower-left triangle region value 1, rest 2
    W=n*run; H=n*rise
    a=np.full((H+2,W+2),2)
    for i in range(H+2):
        for j in range(W+2):
            # row i from top; staircase
            y=H+2-i  # not used
    a=np.full((H,W),2)
    for k in range(n):
        a[k*rise:(k+1)*rise, :(k+1)*run]=1 if False else a[k*rise:(k+1)*rise, :(k+1)*run]
    return a
# simple ones
add('single', [[1]])
add('iso_cell', [[2,2,2],[2,1,2],[2,2,2]])
add('block2', [[2,2,2,2],[2,1,1,2],[2,1,1,2],[2,2,2,2]])
add('corner1', [[1,2],[1,1]])
add('corner3', [[1,2,2],[1,1,2],[1,1,1]])
def tri(n, rx=1, ry=1):
    a=np.full((n*ry,n*rx),2)
    for k in range(n):  # row block k from top has (k+1)*rx cells of 1
        a[k*ry:(k+1)*ry,:(k+1)*rx]=1
    return a
for n in (2,3,4,5,7,10,14):
    add(f'tri{n}', tri(n))
add('tri7_cs30', tri(7), 30)
add('tri3_r2', tri(3,2,1)); add('tri3_r1x2', tri(3,1,2)); add('tri4_r3', tri(4,3,1))
add('tri5_r2', tri(5,2,1))
def circ(r, pad=2):
    n=2*r+2*pad; a=np.full((n,n),2)
    c=n/2
    for i in range(n):
        for j in range(n):
            if (i+0.5-c)**2+(j+0.5-c)**2 <= r*r: a[i,j]=1
    return a
add('circ5', circ(5)); add('circ10', circ(10)); add('circ20', circ(20))
a=np.full((10,20),2); a[5:,:]=1; a[4,8]=1; add('notch_up', a)
a=np.full((10,20),2); a[5:,:]=1; a[4,8:10]=1; add('notch_up2', a)
a=np.full((10,20),2); a[5:,:]=1; a[4,8:11]=1; add('notch_up3', a)
a=np.full((10,20),2); a[5:,:]=1; a[5,8]=2; add('notch_down', a)
a=np.full((10,20),2); a[5:,:]=1; a[4,8:12]=1; a[3,9:11]=1; add('bump2', a)
a=np.full((10,20),2); a[5:,:]=1; a[:5,:10]=3; add('tjunction', a)
a=np.full((10,20),2); a[5:,:]=1; a[4,3]=1; a[4,15]=1; add('notch_two', a)
a=np.full((16,16),2); a[3:13,3:13]=1; add('square10', a)
a=np.full((16,16),2); a[3:13,3:13]=1; a[6:10,6:10]=2; add('square_hole', a)
a=np.full((16,16),2); a[3:13,3:13]=1; a[6:10,6:10]=3; add('square_island', a)
# axis-to-axis staircase like roadmap: region1 = cells with i+j < n (lower-left triangle incl diag)
def tri_axis(n):
    a=np.full((n,n),2)
    for i in range(n):
        for j in range(n):
            # row i from top, y index from bottom = n-1-i
            if j + (n-1-i) < n-1: a[i,j]=1
    return a
add('triaxis7', tri_axis(7), 30); add('triaxis14', tri_axis(14), 30)
a=np.full((12,12),2); a[2:10,2:10]=1; add('square8', a)
a=np.full((8,8),2); a[2:6,2:6]=1; add('square4', a)
a=np.full((8,8),2); a[2:5,2:5]=1; add('square3', a)
a=np.full((8,8),2); a[2:4,2:6]=1; add('rect2x4', a)
a=np.full((6,12),2); a[2:3,2:10]=1; add('line8', a)
def prof(h, H=None):
    h=list(h); H=H or (max(h)+5)
    a=np.full((H,len(h)),2)
    for j,v in enumerate(h): a[H-v:,j]=1
    return a
NEW2=[]
def addp(name,h,H=None): add(name,prof(h,H)); NEW2.append(name)
for H in (1,2,3,4,6):
    addp(f'step{H}', [5]*20+[5+H]*20)
for w in (1,3):
    for H in (2,3,4):
        addp(f'notch_h{H}_w{w}', [5]*10+[5+H]*w+[5]*(20-w))
for k in (2,3,4,6):
    addp(f'ramp{k}', [5]*15+[5+i for i in range(1,k)]+[5+k]*15)
addp('ramp3_run2', [5]*15+[6,6,7,7]+[8]*15)
addp('ramp3_run3', [5]*15+[6,6,6,7,7,7]+[8]*15)
addp('notch_far', [5]*30+[6]+[5]*29)
addp('notch_w5', [5]*10+[6]*5+[5]*15)
addp('notch_w8', [5]*10+[6]*8+[5]*12)
NEW3=[]
def addq(name,h,H=None): add(name,prof(h,H)); NEW3.append(name)
for L in (2,3,4,5,6,7,8,9,10,12,15,20,30):
    addq(f'n1_L{L}', [5]*L+[6]+[5]*L)
    addq(f'n2_L{L}', [5]*L+[7]+[5]*L)
for R in (4,8,11,16,25):
    addq(f'n1_8_{R}', [5]*8+[6]+[5]*R)
NEW4=[]
def addr(name,h,H=None): add(name,prof(h,H)); NEW4.append(name)
for L in (21,22,23,24,25,26,27,28,29,35,40,50):
    addr(f'n1_L{L}', [5]*L+[6]+[5]*L)
addr('n1_30_10',[5]*30+[6]+[5]*10); addr('n1_10_30',[5]*10+[6]+[5]*30)
addr('n1_25_15',[5]*25+[6]+[5]*15); addr('n1_20_30',[5]*20+[6]+[5]*30)
NEW5=[]
def addb(name,h,H=None): add(name,prof(h,H)); NEW5.append(name)
import math as _m
addb('dss_2_5', [5+int(_m.floor(j*2/5)) for j in range(40)])
addb('dss_1_3', [5+int(_m.floor(j/3)) for j in range(45)])
addb('dss_3_4', [5+int(_m.floor(j*3/4)) for j in range(28)])
addb('bend_13_11', [5+int(_m.floor(j/3)) for j in range(30)]+[15+k for k in range(1,11)])
addb('bend_11_13', [5+k for k in range(10)]+[15+int(_m.floor(j/3)) for j in range(30)])
addb('bend_13_0', [5+int(_m.floor(j/3)) for j in range(30)]+[15]*15)
addb('arc_par', [5+int(_m.floor(20-((j-20)**2)/20)) for j in range(41)])
addb('arc_par2', [5+int(_m.floor(12-((j-15)**2)/18)) for j in range(31)])
