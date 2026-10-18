import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/ss'
sr = arcpy.SpatialReference(32632)
fc = arcpy.management.CreateFeatureclass(P(D), 'cfs.shp', 'POINT', spatial_reference=sr)
for f in ('W', 'S'): arcpy.management.AddField(fc, f, 'DOUBLE')
pts = [(0, 0, 1, 0), (10, 0, 2, 0), (20, 0, 1, 0), (35, 5, 1, 0)]
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'W', 'S']) as c:
    for x, y, w, s in pts: c.insertRow([(500000 + x, 5000000 + y), w, s])
src = P(D) + '/cfs.shp'
res = {}
def pick(tag, **kw):
    o = P(D) + '/ocf_%s.shp' % tag
    arcpy.stats.CentralFeature(src, o, 'EUCLIDEAN_DISTANCE', **kw)
    with arcpy.da.SearchCursor(o, ['SHAPE@XY', 'W', 'S']) as cur:
        r = list(cur)[0]
    res[tag] = r; print(tag, r)
pick('plain')
# vary self potential of each feature via update
for tag, svals in {'s_b100': [0, 100, 0, 0], 's_b3': [0, 3, 0, 0], 's_b10': [0, 10, 0, 0], 's_all_ramp': [50, 0, 50, 0], 's_b25': [0, 25, 0, 0], 's_b40': [0, 40, 0, 0]}.items():
    with arcpy.da.UpdateCursor(src, ['S']) as cur:
        for i, r in enumerate(cur):
            cur.updateRow([svals[i]])
    pick(tag + '_w', Weight_Field='W', Self_Potential_Weight_Field='S')
    pick(tag, Self_Potential_Weight_Field='S')
