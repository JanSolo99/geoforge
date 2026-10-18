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
    go(ds + ':fixed250', Conceptualization_of_Spatial_Relationships='FIXED_DISTANCE_BAND', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=250, **base)
    go(ds + ':fixed250_row', Conceptualization_of_Spatial_Relationships='FIXED_DISTANCE_BAND', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='ROW', Distance_Band_or_Threshold_Distance=250, **base)
    go(ds + ':knn', Conceptualization_of_Spatial_Relationships='K_NEAREST_NEIGHBORS', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Number_of_Neighbors=5, **base)
    go(ds + ':inv', Conceptualization_of_Spatial_Relationships='INVERSE_DISTANCE', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='ROW', Distance_Band_or_Threshold_Distance=0, **base)
    go(ds + ':zoi', Conceptualization_of_Spatial_Relationships='ZONE_OF_INDIFFERENCE', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='ROW', Distance_Band_or_Threshold_Distance=250, **base)
    go(ds + ':fixed250_fdr', Conceptualization_of_Spatial_Relationships='FIXED_DISTANCE_BAND', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=250, Apply_False_Discovery_Rate__FDR__Correction='APPLY_FDR', **base)
    go(ds + ':fixed0', Conceptualization_of_Spatial_Relationships='FIXED_DISTANCE_BAND', Distance_Method='EUCLIDEAN_DISTANCE', Standardization='NONE', Distance_Band_or_Threshold_Distance=0, **base)
json.dump(out, open(P(D) + '/ss7.json', 'w'), default=str)
