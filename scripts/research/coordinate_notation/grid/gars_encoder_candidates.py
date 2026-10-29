"""Evaluate arithmetic hypotheses against numeric-input ArcGIS observations."""
import collections
import json
import math
from pathlib import Path

HERE = Path(__file__).resolve().parent
LETTERS = 'ABCDEFGHJKLMNPQRSTUVWXYZ'


def ticks(code):
    q, k = int(code[5]), int(code[6])-1
    x = (int(code[:3])-1)*6 + (3 if q in (2,4) else 0) + k%3
    y = (LETTERS.index(code[3])*24+LETTERS.index(code[4]))*6 + (3 if q in (1,2) else 0) + 2-k//3
    return x, y


def staged(v, offset):
    x = (v+offset)*2
    i = min(math.floor(x),359) if offset==90 else math.floor(x)
    fraction=x-i
    east=fraction>=.5
    local=(fraction-(.5 if east else 0))*2
    index=min(math.floor(local*3),2)
    return i*6+3*east+index


def main():
    rows=json.loads((HERE/'gars_numeric_results.json').read_text())
    print('Parsed CSV values changed:',sum(r['x']!=r['input_x'] or r['y']!=r['input_y'] for r in rows))
    print('Grid-vs-numeric centers:',collections.Counter((r['variant'],r['numeric_output']==r['grid_output']) for r in rows if r['variant'] in ('rational','naive')))
    for axis,offset in [('x',180),('y',90)]:
        funcs={
            'serialize15g_roundtrip_ticks':lambda v:math.floor((math.degrees(math.radians(float(format(v,'.15g'))))+offset)*12),
            'staged':lambda v:staged(v,offset),
            'ticks':lambda v:math.floor((v+offset)*12),
            'scale_first':lambda v:math.floor(v*12+offset*12),
            'staged_radian_roundtrip':lambda v:staged(math.degrees(math.radians(v)),offset),
            'scale_first_radian_roundtrip':lambda v:math.floor(math.degrees(math.radians(v))*12+offset*12),
        }
        for name,fn in funcs.items():
            bad=[]
            for row in rows:
                expected=ticks(row['numeric_output'])[0 if axis=='x' else 1]
                got=fn(row['input_'+axis])
                got=got%4320 if axis=='x' else min(got,2159)
                if got!=expected: bad.append((row['code'],row['variant'],row['input_'+axis],got,expected))
            print(axis,name,len(bad),bad[:3])


if __name__=='__main__': main()
