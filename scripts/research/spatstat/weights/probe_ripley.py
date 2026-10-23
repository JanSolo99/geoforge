import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/sw2'
S = arcpy.stats; out = {}
bs, ps = P(D) + '/big.shp', P(D) + '/pts.shp'
# study area polygon
sa = arcpy.management.CreateFeatureclass(P(D), 'sa.shp', 'POLYGON', spatial_reference=arcpy.SpatialReference(32632))
with arcpy.da.InsertCursor(sa, ['SHAPE@']) as c:
    c.insertRow([arcpy.Polygon(arcpy.Array([arcpy.Point(*p) for p in [(499900, 4999900), (499950, 5001600), (502100, 5001700), (502050, 4999950), (499900, 4999900)]]), arcpy.SpatialReference(32632))])
cnt = [0]
def go(key, src, nb, **kw):
    cnt[0] += 1; o = P(D) + '/rk_%d.dbf' % cnt[0]
    try:
        r = S.MultiDistanceSpatialClustering(src, o, nb, '0_PERMUTATIONS_-_NO_CONFIDENCE_ENVELOPE', 'NO_DISPLAY', **kw)
        d = arcpy.Describe(o)
        out[key] = {'fields': [(f.name, f.type, f.length, f.precision, f.scale) for f in d.fields], 'rows': [list(x) for x in arcpy.da.SearchCursor(o, [f.name for f in d.fields])], 'msg': r.getMessages()}
        print('ok', key, out[key]['rows'][:2])
    except Exception as e: print('FAIL', key, str(e)[:300])
go('big', bs, 10)
go('big_w', bs, 10, Weight_Field='V')
go('big_b_i', bs, 5, Beginning_Distance=100, Distance_Increment=50)
go('big_sim', bs, 8, Boundary_Correction_Method='SIMULATE_OUTER_BOUNDARY_VALUES')
go('big_red', bs, 8, Boundary_Correction_Method='REDUCE_ANALYSIS_AREA')
go('big_rip', bs, 8, Boundary_Correction_Method='RIPLEY_EDGE_CORRECTION_FORMULA')
go('big_sa', bs, 8, Study_Area_Method='USER_PROVIDED_STUDY_AREA_FEATURE_CLASS', Study_Area_Feature_Class=P(D) + '/sa.shp')
go('big_sa_rip', bs, 8, Boundary_Correction_Method='RIPLEY_EDGE_CORRECTION_FORMULA', Study_Area_Method='USER_PROVIDED_STUDY_AREA_FEATURE_CLASS', Study_Area_Feature_Class=P(D) + '/sa.shp')
go('pts', ps, 6)
go('pts_w_rip', ps, 6, Weight_Field='V', Boundary_Correction_Method='RIPLEY_EDGE_CORRECTION_FORMULA')
go('pts_w_sim', ps, 6, Weight_Field='V', Boundary_Correction_Method='SIMULATE_OUTER_BOUNDARY_VALUES')
go('pts_w_red', ps, 6, Weight_Field='V', Boundary_Correction_Method='REDUCE_ANALYSIS_AREA')
go('pg', P(D) + '/bpg.shp', 6)
try:
    o = P(D) + '/rk_env.dbf'
    S.MultiDistanceSpatialClustering(bs, o, 4, '9_PERMUTATIONS', 'NO_DISPLAY')
    d = arcpy.Describe(o); out['env'] = {'fields': [f.name for f in d.fields], 'rows': [list(x) for x in arcpy.da.SearchCursor(o, [f.name for f in d.fields])]}
    print(out['env'])
except Exception as e: print('FAIL env', e)
json.dump(out, open(P(D) + '/rk.json', 'w'))
