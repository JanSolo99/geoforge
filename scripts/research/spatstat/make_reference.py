import arcpy, json, random, math
arcpy.env.overwriteOutput = True
arcpy.env.extent = arcpy.env.snapRaster = arcpy.env.mask = None
D = '/tmp/claude-1000/ss'
sr = arcpy.SpatialReference(32632)
out = {'datasets': {}, 'cases': {}}
def mkpts(name, seed, n, groups):
    random.seed(seed)
    pts = [(round(500000 + random.gauss(300, 150), 3), round(5000000 + random.gauss(200, 100), 3), round(random.uniform(1, 10), 3), random.choice(groups), random.randint(0, 50)) for _ in range(n)]
    fc = arcpy.management.CreateFeatureclass(P(D), name + '.shp', 'POINT', spatial_reference=sr)
    arcpy.management.AddField(fc, 'W', 'DOUBLE'); arcpy.management.AddField(fc, 'G', 'TEXT', field_length=4); arcpy.management.AddField(fc, 'Z', 'LONG')
    with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'W', 'G', 'Z']) as c:
        for x, y, w, g, z in pts: c.insertRow([(x, y), w, g, z])
    out['datasets'][name] = {'kind': 'point', 'rows': pts}
    return P(D) + '/' + name + '.shp'
def mklines(name, seed, n, groups):
    random.seed(seed)
    ls = []
    fc = arcpy.management.CreateFeatureclass(P(D), name + '.shp', 'POLYLINE', spatial_reference=sr)
    arcpy.management.AddField(fc, 'G', 'TEXT', field_length=4)
    with arcpy.da.InsertCursor(fc, ['SHAPE@', 'G']) as c:
        for i in range(n):
            x0, y0 = round(500000 + random.uniform(0, 600), 3), round(5000000 + random.uniform(0, 400), 3)
            ang = math.radians(random.gauss(100, 90)); ln = random.uniform(30, 200)
            x1, y1 = round(x0 + ln * math.sin(ang), 3), round(y0 + ln * math.cos(ang), 3)
            g = random.choice(groups); ls.append([x0, y0, x1, y1, g])
            c.insertRow([arcpy.Polyline(arcpy.Array([arcpy.Point(x0, y0), arcpy.Point(x1, y1)]), sr), g])
    out['datasets'][name] = {'kind': 'line', 'rows': ls}
    return P(D) + '/' + name + '.shp'
def dump(path):
    d = arcpy.Describe(path)
    names = [f.name for f in d.fields if f.type != 'Geometry']
    fields = [(f.name, f.type, f.length, f.precision) for f in d.fields]
    rows = []
    with arcpy.da.SearchCursor(path, names + ['SHAPE@JSON']) as cur:
        for r in cur:
            rows.append({'attrs': list(r[:-1]), 'geom': json.loads(r[-1])})
    return {'fields': fields, 'rows': rows}
cnt = [0]
def run(key, ds, fn, **kw):
    cnt[0] += 1
    o = P(D) + '/f_%d.shp' % cnt[0]
    try:
        fn(ds, o, **kw)
        out['cases'][key] = dump(o); print('ok', key)
    except Exception as e:
        print('FAIL', key, str(e)[:120])
for dsname, src in [('A', mkpts('fa', 21, 40, ['a', 'b', 'c'])), ('B', mkpts('fb', 33, 70, ['p', 'q', 'r', 's', 'u']))]:
    S = arcpy.stats
    run(dsname + ':mean', src, S.MeanCenter)
    run(dsname + ':mean_w', src, S.MeanCenter, Weight_Field='W')
    run(dsname + ':mean_case', src, S.MeanCenter, Weight_Field='W', Case_Field='G')
    run(dsname + ':mean_dim', src, S.MeanCenter, Weight_Field='W', Case_Field='G', Dimension_Field='Z')
    run(dsname + ':median', src, S.MedianCenter)
    run(dsname + ':median_w', src, S.MedianCenter, Weight_Field='W')
    run(dsname + ':median_case', src, S.MedianCenter, Weight_Field='W', Case_Field='G')
    run(dsname + ':central', src, S.CentralFeature, Distance_Method='EUCLIDEAN_DISTANCE')
    run(dsname + ':central_man_w', src, S.CentralFeature, Distance_Method='MANHATTAN_DISTANCE', Weight_Field='W')
    run(dsname + ':central_self', src, S.CentralFeature, Distance_Method='EUCLIDEAN_DISTANCE', Weight_Field='W', Self_Potential_Weight_Field='Z')
    run(dsname + ':central_case', src, S.CentralFeature, Distance_Method='EUCLIDEAN_DISTANCE', Weight_Field='W', Case_Field='G')
    run(dsname + ':sd1', src, S.StandardDistance, Circle_Size='1_STANDARD_DEVIATION')
    run(dsname + ':sd2_w', src, S.StandardDistance, Circle_Size='2_STANDARD_DEVIATIONS', Weight_Field='W')
    run(dsname + ':sd3_case', src, S.StandardDistance, Circle_Size='3_STANDARD_DEVIATIONS', Weight_Field='W', Case_Field='G')
    run(dsname + ':ell1', src, S.DirectionalDistribution, Ellipse_Size='1_STANDARD_DEVIATION')
    run(dsname + ':ell2_w', src, S.DirectionalDistribution, Ellipse_Size='2_STANDARD_DEVIATIONS', Weight_Field='W')
    run(dsname + ':ell3_case', src, S.DirectionalDistribution, Ellipse_Size='3_STANDARD_DEVIATIONS', Weight_Field='W', Case_Field='G')
lsrc = mklines('fl', 5, 40, ['m', 'n', 'o'])
run('L:ldm', lsrc, arcpy.stats.DirectionalMean, Orientation_Only='DIRECTION')
run('L:ldm_o', lsrc, arcpy.stats.DirectionalMean, Orientation_Only='ORIENTATION_ONLY')
run('L:ldm_case', lsrc, arcpy.stats.DirectionalMean, Orientation_Only='DIRECTION', Case_Field='G')
run('L:ldm_o_case', lsrc, arcpy.stats.DirectionalMean, Orientation_Only='ORIENTATION_ONLY', Case_Field='G')
json.dump(out, open(P(D) + '/final.json', 'w'))
