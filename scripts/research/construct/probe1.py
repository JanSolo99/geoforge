import arcpy, json, random, math
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
D = '/tmp/claude-1000/vt'
sr = arcpy.SpatialReference(32632)
random.seed(8)
def mk(name, kind, rows, fields):
    fc = arcpy.management.CreateFeatureclass(P(D), name + '.shp', kind, spatial_reference=sr)
    for f, t in fields: arcpy.management.AddField(fc, f, t)
    with arcpy.da.InsertCursor(fc, ['SHAPE@'] + [f for f, t in fields]) as c:
        for g, *a in rows: c.insertRow([g] + a)
    return P(D) + '/' + name + '.shp'
X0, Y0 = 500000, 5000000
def poly(rings):
    return arcpy.Polygon(arcpy.Array([arcpy.Array([arcpy.Point(X0 + x, Y0 + y) for x, y in r]) for r in rings]), sr)
def line(pts_list):
    return arcpy.Polyline(arcpy.Array([arcpy.Array([arcpy.Point(X0 + x, Y0 + y) for x, y in pts]) for pts in pts_list]), sr)
polys = mk('vp', 'POLYGON', [
    (poly([[(0, 0), (0, 100), (100, 100), (100, 0), (0, 0)]]), 1, 'a'),
    (poly([[(100, 0), (100, 100), (200, 100), (200, 0), (100, 0)]]), 2, 'b'),
    (poly([[(300, 0), (300, 150), (450, 150), (450, 0), (300, 0)], [(330, 30), (370, 30), (370, 70), (330, 70), (330, 30)]]), 3, 'c'),
    (poly([[(0, 200), (50, 300), (150, 260), (120, 190), (0, 200)]]), 4, 'a'),
], [('N', 'LONG'), ('G', 'TEXT')])
lines = mk('vl', 'POLYLINE', [
    (line([[(0, 0), (60, 20), (130, 10)]]), 1, 'a'),
    (line([[(0, 50), (100, 50)], [(120, 50), (200, 80), (250, 60)]]), 2, 'b'),
    (line([[(10, 120), (40, 190), (100, 200), (180, 150)]]), 3, 'a'),
], [('N', 'LONG'), ('G', 'TEXT')])
pts = mk('vpt', 'POINT', [(arcpy.PointGeometry(arcpy.Point(X0 + round(random.uniform(0, 500), 3), Y0 + round(random.uniform(0, 400), 3)), sr), i, random.choice('ab')) for i in range(14)], [('N', 'LONG'), ('G', 'TEXT')])
out = {}
def pyval(v):
    if hasattr(v, 'JSON'): return json.loads(v.JSON)
    return v
def dump(path):
    d = arcpy.Describe(path)
    names = [f.name for f in d.fields if f.type != 'Geometry']
    fields = [(f.name, f.type, f.length, f.precision) for f in d.fields]
    rows = []
    with arcpy.da.SearchCursor(path, names + ['SHAPE@JSON']) as cur:
        for r in cur: rows.append({'attrs': list(r[:-1]), 'geom': json.loads(r[-1])})
    return {'fields': fields, 'rows': rows}
cnt = [0]
def run(key, fn, copy=None):
    cnt[0] += 1
    o = P(D) + '/o%d.shp' % cnt[0]
    try:
        if copy:
            arcpy.management.CopyFeatures(copy, o); fn(o)
        else:
            fn(o)
        out[key] = dump(o); print('ok', key, len(out[key]['rows']), [f[0] for f in out[key]['fields']])
    except Exception as e:
        print('FAIL', key, str(e)[:150])
out['data'] = {'polys': 'see vp', 'lines': 'see vl'}
# densify
for tag, kw in {'d15': dict(densification_method='DISTANCE', distance=15), 'd7_5': dict(densification_method='DISTANCE', distance=7.5), 'd40': dict(densification_method='DISTANCE', distance=40)}.items():
    run('densify_lines_' + tag, lambda o, kw=kw: arcpy.edit.Densify(o, **kw), copy=lines)
    run('densify_polys_' + tag, lambda o, kw=kw: arcpy.edit.Densify(o, **kw), copy=polys)
# points along lines
run('gpal_d25', lambda o: arcpy.management.GeneratePointsAlongLines(lines, o, 'DISTANCE', Distance='25 Meters', Include_End_Points='NO_END_POINTS'))
run('gpal_d25_end', lambda o: arcpy.management.GeneratePointsAlongLines(lines, o, 'DISTANCE', Distance='25 Meters', Include_End_Points='END_POINTS'))
run('gpal_p20', lambda o: arcpy.management.GeneratePointsAlongLines(lines, o, 'PERCENTAGE', Percentage=20, Include_End_Points='NO_END_POINTS'))
run('gpal_p20_end', lambda o: arcpy.management.GeneratePointsAlongLines(lines, o, 'PERCENTAGE', Percentage=20, Include_End_Points='END_POINTS'))
run('gpal_d70_polys', lambda o: arcpy.management.GeneratePointsAlongLines(polys, o, 'DISTANCE', Distance='70 Meters', Include_End_Points='NO_END_POINTS'))
# fishnet
run('fish', lambda o: arcpy.management.CreateFishnet(o, '%f %f' % (X0, Y0), '%f %f' % (X0, Y0 + 10), 50, 40, 4, 5, '#', 'NO_LABELS', '#', 'POLYGON'))
run('fish_ext', lambda o: arcpy.management.CreateFishnet(o, '%f %f' % (X0, Y0), '%f %f' % (X0, Y0 + 10), 0, 0, 4, 5, '%f %f' % (X0 + 200, Y0 + 100), 'NO_LABELS', '#', 'POLYLINE'))
run('fish_corner', lambda o: arcpy.management.CreateFishnet(o, '%f %f' % (X0, Y0), '%f %f' % (X0 + 10, Y0), 30, 30, 3, 3, '%f %f' % (X0 + 90, Y0 + 90), 'NO_LABELS', '#', 'POLYGON'))
# polygon to line
run('p2l_neigh', lambda o: arcpy.management.PolygonToLine(polys, o, 'IDENTIFY_NEIGHBORS'))
run('p2l_ign', lambda o: arcpy.management.PolygonToLine(polys, o, 'IGNORE_NEIGHBORS'))
# multiple ring buffer
run('mrb', lambda o: arcpy.analysis.MultipleRingBuffer(pts, o, [20, 50, 80], 'Meters', 'distance', 'ALL', 'FULL'))
run('mrb_none', lambda o: arcpy.analysis.MultipleRingBuffer(pts, o, [20, 50], 'Meters', 'distance', 'NONE', 'FULL'))
run('mrb_out', lambda o: arcpy.analysis.MultipleRingBuffer(pts, o, [20, 50], 'Meters', 'distance', 'ALL', 'OUTSIDE_ONLY'))
# thiessen
run('thies', lambda o: arcpy.analysis.CreateThiessenPolygons(pts, o, 'ALL'))
run('thies_fid', lambda o: arcpy.analysis.CreateThiessenPolygons(pts, o, 'ONLY_FID'))
json.dump(out, open(P(D) + '/vt1.json', 'w'))
