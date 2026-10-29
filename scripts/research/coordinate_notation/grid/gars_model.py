"""Explicit arithmetic hypotheses calibrated solely against ArcGIS black-box data."""
import math

LETTERS = 'ABCDEFGHJKLMNPQRSTUVWXYZ'


def parse_grid(fmt, text):
    s=''.join((text or '').upper().split())
    try:
        if fmt=='GARS':
            if len(s)<5 or not s[:3].isdigit(): return None
            band=int(s[:3]); row=LETTERS.index(s[3])*24+LETTERS.index(s[4])
            if not 1<=band<=720 or row>=360: return None
            dx=dy=0
            if len(s)>5:
                q=int(s[5])
                if not 1<=q<=4:return None
                dx+=3*(q in (2,4));dy+=3*(q in (1,2))
            if len(s)>6:
                k=int(s[6])-1
                if not 0<=k<9:return None
                dx+=k%3;dy+=2-k//3
            return ((band-1)*6+dx)/12-180,(row*6+dy)/12-90
        if fmt=='GEOREF':
            if not 8<=len(s)<=22 or len(s)%2:return None
            xt,yt,xd,yd=(LETTERS.index(c) for c in s[:4])
            if yt>=12 or xd>=15 or yd>=15:return None
            n=(len(s)-4)//2;scale=60*10**(n-2)
            if not s[4:].isdigit():return None
            return xt*15+xd-180+int(s[4:4+n])/scale,yt*15+yd-90+int(s[4+n:])/scale
    except (ValueError,IndexError): pass
    return None


def encode_grid(position):
    if position is None:return ''
    x,y=position
    if abs(x)>180 or abs(y)>90:return ''
    x=math.floor((math.degrees(math.radians(x))+180)*12)%4320
    y=min(max(math.floor((math.degrees(math.radians(y))+90)*12),0),2159)
    quadrant={(True,False):1,(True,True):2,(False,False):3,(False,True):4}[(y%6>=3,x%6>=3)]
    key=(2-y%3)*3+x%3+1
    return f'{x//6+1:03}{LETTERS[y//6//24]}{LETTERS[y//6%24]}{quadrant}{key}'
