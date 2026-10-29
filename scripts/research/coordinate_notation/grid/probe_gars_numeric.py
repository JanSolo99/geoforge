"""Separate GARS encoder behavior from decoder rounding using numeric f64 corners."""
import csv
import json
import math

root = '/mnt/media/ArcGIS_Linux/scripts/research/coordinate_notation/grid'
with open(P(root + '/probe_boundaries_results.txt')) as f:
    controls = json.loads(f.readline().split(' ', 1)[1])
letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ'
rows = []
for code, expected in controls:
    band = int(code[:3])-1
    row = letters.index(code[3])*24+letters.index(code[4])
    q, k = int(code[5]), int(code[6])-1
    dx, dy = (3 if q in (2,4) else 0)+k%3, (3 if q in (1,2) else 0)+(2-k//3)
    x, y = (band*6+dx-2160)/12, (row*6+dy-1080)/12
    naive_x = band/2-180 + (0.25 if q in (2,4) else 0) + (k%3)/12
    naive_y = row/2-90 + (0.25 if q in (1,2) else 0) + (2-k//3)/12
    variants = [('rational',x,y), ('naive',naive_x,naive_y),
                ('west',math.nextafter(x,-math.inf),y), ('east',math.nextafter(x,math.inf),y),
                ('south',x,math.nextafter(y,-math.inf)), ('north',x,math.nextafter(y,math.inf)),
                ('southwest',math.nextafter(x,-math.inf),math.nextafter(y,-math.inf)),
                ('northeast',math.nextafter(x,math.inf),math.nextafter(y,math.inf))]
    for label, lon, lat in variants:
        if abs(lon)<=180 and abs(lat)<=90:
            rows.append({'id':len(rows)+1,'code':code,'variant':label,'x':lon,'y':lat,'grid_output':expected})
source = '/tmp/unigis-gars-numeric.csv'
with open(P(source),'w',newline='') as f:
    w=csv.writer(f);w.writerow(['ID','LON','LAT'])
    w.writerows((r['id'],format(r['x'],'.17g'),format(r['y'],'.17g')) for r in rows)
out = 'memory/gars_numeric'
arcpy.management.ConvertCoordinateNotation(P(source),out,'LON','LAT','DD_2','GARS','',arcpy.SpatialReference(4326))
with arcpy.da.SearchCursor(out,['ID','LON','LAT','GARS']) as cursor:
    for index,lon,lat,value in cursor:
        rows[index-1].update({'input_x':lon,'input_y':lat,'numeric_output':value})
with open(P('/tmp/unigis-gars-numeric-results.json'),'w') as f: json.dump(rows,f)
print('Saved',len(rows),'numeric cases to /tmp/unigis-gars-numeric-results.json')
