import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/ss'
out = {}
def vals(r): return [r.getOutput(i) for i in range(r.outputCount)]
def go(key, fn):
    try:
        r = fn(); out[key] = vals(r); print('ok', key, out[key][:6])
    except Exception as e:
        print('FAIL', key, str(e)[:140])
for ds in ('fa', 'fb'):
    src = P(D) + '/%s.shp' % ds
    go(ds + ':ann', lambda: arcpy.stats.AverageNearestNeighbor(src, 'EUCLIDEAN_DISTANCE', 'NO_REPORT', '#'))
    go(ds + ':ann_man', lambda: arcpy.stats.AverageNearestNeighbor(src, 'MANHATTAN_DISTANCE', 'NO_REPORT', '#'))
    go(ds + ':ann_area', lambda: arcpy.stats.AverageNearestNeighbor(src, 'EUCLIDEAN_DISTANCE', 'NO_REPORT', 400000))
    for conc in ['INVERSE_DISTANCE', 'INVERSE_DISTANCE_SQUARED', 'FIXED_DISTANCE_BAND', 'ZONE_OF_INDIFFERENCE', 'K_NEAREST_NEIGHBORS']:
        for std in ('NONE', 'ROW'):
            for dm in ('EUCLIDEAN_DISTANCE', 'MANHATTAN_DISTANCE'):
                if dm == 'MANHATTAN_DISTANCE' and std == 'NONE': continue
                go('%s:moran:%s:%s:%s' % (ds, conc, std, dm), lambda conc=conc, std=std, dm=dm: arcpy.stats.SpatialAutocorrelation(src, 'W', 'NO_REPORT', conc, dm, std, 0 if conc != 'K_NEAREST_NEIGHBORS' else 6))
    go(ds + ':moran:fixed250:NONE', lambda: arcpy.stats.SpatialAutocorrelation(src, 'W', 'NO_REPORT', 'FIXED_DISTANCE_BAND', 'EUCLIDEAN_DISTANCE', 'NONE', 250))
    go(ds + ':moran:inv_band300', lambda: arcpy.stats.SpatialAutocorrelation(src, 'W', 'NO_REPORT', 'INVERSE_DISTANCE', 'EUCLIDEAN_DISTANCE', 'NONE', 300))
    go(ds + ':moran:zoi250', lambda: arcpy.stats.SpatialAutocorrelation(src, 'W', 'NO_REPORT', 'ZONE_OF_INDIFFERENCE', 'EUCLIDEAN_DISTANCE', 'ROW', 250))
    go(ds + ':moran:knn3', lambda: arcpy.stats.SpatialAutocorrelation(src, 'W', 'NO_REPORT', 'K_NEAREST_NEIGHBORS', 'EUCLIDEAN_DISTANCE', 'ROW', 3))
json.dump(out, open(P(D) + '/ss6.json', 'w'), default=str)
