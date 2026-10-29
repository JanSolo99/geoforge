"""Independent GARS subcells and GEOREF precision/edge controls."""
import csv
import random
from pathlib import Path

HERE = Path(__file__).resolve().parent
LETTERS = 'ABCDEFGHJKLMNPQRSTUVWXYZ'


def gars(band, row, quadrant=None, key=None):
    value = f'{band:03}{LETTERS[row//24]}{LETTERS[row%24]}'
    return value + (str(quadrant) if quadrant else '') + (str(key) if key else '')


def georef(x, y, xm, ym, digits):
    a, b = x+180, y+90
    return f'{LETTERS[a//15]}{LETTERS[b//15]}{LETTERS[a%15]}{LETTERS[b%15]}{xm:0{digits}d}{ym:0{digits}d}'


def main():
    rng = random.Random(91762)
    values = [gars(rng.randrange(1,721),rng.randrange(360),rng.randrange(1,5),rng.randrange(1,10))
              for _ in range(480)]
    for band,row in [(1,0),(77,35),(116,255),(720,359)]:
        values.append(gars(band,row))
        for q in range(1,5):
            values.append(gars(band,row,q))
            values.extend(gars(band,row,q,k) for k in range(1,10))
    values += ['391 mh 32 ', '391MH31junk', '000AA11','721AA11','391MH0','391MH30','junk','']
    groups = {'GARS':values}
    values=[]
    for _ in range(480):
        digits=rng.randrange(2,10)
        scale=10**(digits-2)
        def component():
            return max(0,min(60*scale-1,5*rng.randrange(12)*scale+rng.choice([-1,0,1])))
        values.append(georef(rng.randrange(-180,180),rng.randrange(-90,90),component(),component(),digits))
    values += ['AAAA0000','ZMQQ6060','ZMQQ9999999999','PKAA050000000400000000',
               'PKAA049999999400000000','PKAA050000001400000000','PKAA','PKAA07',
               'ZZAA0000','PKAA-07404073','junk','']
    groups['GEOREF']=values
    for fmt,values in groups.items():
        path=HERE/f'gars_boundary_{fmt.lower()}.csv'
        with path.open('w',newline='') as f:
            writer=csv.writer(f);writer.writerow(['ID','COORD']);writer.writerows(enumerate(values,1))
        print(path.name,len(values),'rows')


if __name__=='__main__': main()
