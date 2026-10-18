import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/vt'
sr = arcpy.SpatialReference(32632)
X0, Y0 = 500000, 5000000
def line(parts): return arcpy.Polyline(arcpy.Array([arcpy.Array([arcpy.Point(X0 + x, Y0 + y) for x, y in p]) for p in parts]), sr)
def poly(rings): return arcpy.Polygon(arcpy.Array([arcpy.Array([arcpy.Point(X0 + x, Y0 + y) for x, y in r]) for r in rings]), sr)
fc = arcpy.management.CreateFeatureclass(P(D), 'vl2.shp', 'POLYLINE', spatial_reference=sr)
arcpy.management.AddField(fc, 'N', 'LONG')
rows = [
 (line([[(0, 0), (70, 0)], [(100, 0), (100, 60)]]), 1),          # parts of length 70 and 60
 (line([[(0, 20), (10, 20)]]), 2),                                # shorter than the distance
 (line([[(0, 40), (50, 40)]]), 3),                                # exactly two distances
 (line([[(0, 60), (30, 60), (30, 90)], [(40, 90), (40, 120)], [(60, 120), (90, 120)]]), 4),  # three parts of 60 / 30 / 30
]
with arcpy.da.InsertCursor(fc, ['SHAPE@', 'N']) as c:
    for g, n in rows: c.insertRow([g, n])
pc = arcpy.management.CreateFeatureclass(P(D), 'vq2.shp', 'POLYGON', spatial_reference=sr)
arcpy.management.AddField(pc, 'N', 'LONG')
with arcpy.da.InsertCursor(pc, ['SHAPE@', 'N']) as c:
    c.insertRow([poly([[(0, 0), (0, 40), (60, 40), (60, 0), (0, 0)]]), 1])
out = {}
def dump(path):
    d = arcpy.Describe(path); names = [f.name for f in d.fields if f.type != 'Geometry']
    rows = []
    with arcpy.da.SearchCursor(path, names + ['SHAPE@JSON']) as cur:
        for r in cur: rows.append({'attrs': list(r[:-1]), 'geom': json.loads(r[-1])})
    return {'fields': [(f.name, f.type, f.length, f.precision) for f in d.fields], 'rows': rows}
cnt = [0]
def go(key, src, **kw):
    cnt[0] += 1; o = P(D) + '/g2_%d.shp' % cnt[0]
    try:
        arcpy.management.GeneratePointsAlongLines(P(D) + '/' + src + '.shp', o, **kw); out[key] = dump(o)
        print('ok', key, [(round(r['geom']['x'] - X0, 3), round(r['geom']['y'] - Y0, 3)) for r in out[key]['rows']])
    except Exception as e: print('FAIL', key, str(e)[:100])
for e in ('NO_END_POINTS', 'END_POINTS'):
    go('d25_' + e, 'vl2', Point_Placement='DISTANCE', Distance='25 Meters', Include_End_Points=e)
    go('d25_polys_' + e, 'vq2', Point_Placement='DISTANCE', Distance='25 Meters', Include_End_Points=e)
    go('p25_' + e, 'vl2', Point_Placement='PERCENTAGE', Percentage=25, Include_End_Points=e)
    go('p50_polys_' + e, 'vq2', Point_Placement='PERCENTAGE', Percentage=50, Include_End_Points=e)
go('d0_5km', 'vl2', Point_Placement='DISTANCE', Distance='0.025 Kilometers', Include_End_Points='NO_END_POINTS')
json.dump(out, open(P(D) + '/vt2.json', 'w'))
