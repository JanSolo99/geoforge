import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/ss'
out = {}
def dump(path):
    d = arcpy.Describe(path); names = [f.name for f in d.fields if f.type != 'Geometry']
    rows = []
    with arcpy.da.SearchCursor(path, names) as cur:
        for r in cur: rows.append(list(r))
    return {'fields': [(f.name, f.type, f.length, f.precision) for f in d.fields], 'rows': rows}
cnt = [0]
def go(key, **kw):
    cnt[0] += 1; o = P(D) + '/hs%d.shp' % cnt[0]
    try:
        arcpy.stats.HotSpots(Output_Feature_Class=o, **kw); out[key] = dump(o)
        print('ok', key, [f[0] for f in out[key]['fields']], out[key]['rows'][0])
    except Exception as e: print('FAIL', key, str(e)[:140])
for ds in ('fa', 'fb'):
    src = P(D) + '/%s.shp' % ds
    base = dict(Input_Feature_Class=src, Input_Field='W')
    for k in (5, 12):
        go('%s:knn%d' % (ds, k), Conceptualization_of_Spatial_Relationships='K_NEAREST_NEIGHBORS', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=0, number_of_neighbors=k, **base)
    go(ds + ':knn5_man', Conceptualization_of_Spatial_Relationships='K_NEAREST_NEIGHBORS', Distance_Method='MANHATTAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=0, number_of_neighbors=5, **base)
    go(ds + ':invsq', Conceptualization_of_Spatial_Relationships='INVERSE_DISTANCE_SQUARED', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=0, **base)
    go(ds + ':inv_band', Conceptualization_of_Spatial_Relationships='INVERSE_DISTANCE', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=200, **base)
json.dump(out, open(P(D) + '/ss8.json', 'w'), default=str)
