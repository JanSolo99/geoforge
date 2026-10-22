import arcpy, json
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/sw2'
S = arcpy.stats; bs, bg = P(D) + '/big.shp', P(D) + '/bpg.shp'
out = {}
def vals(r): return [r.getOutput(i) for i in range(3)]
for key, src, conc, band in [('bpg:inv:0', bg, 'INVERSE_DISTANCE', 0), ('bpg:fix:none', bg, 'FIXED_DISTANCE_BAND', None), ('big:fix:none', bs, 'FIXED_DISTANCE_BAND', None), ('big:zoi:none', bs, 'ZONE_OF_INDIFFERENCE', None), ('big:inv:none', bs, 'INVERSE_DISTANCE', None)]:
    try:
        r = S.SpatialAutocorrelation(src, 'V', 'NO_REPORT', conc, 'EUCLIDEAN_DISTANCE', 'ROW', band)
        out[key] = vals(r); print(key, out[key], [m for m in r.getMessages().split('\n') if 'WARN' in m])
    except Exception as e: print('FAIL', key, str(e)[:200])
o = P(D) + '/hs_none.shp'
S.HotSpots(bs, 'V', o, 'FIXED_DISTANCE_BAND', 'EUCLIDEAN_DISTANCE', 'NONE', None)
out['hot:fix:none'] = [list(r) for r in arcpy.da.SearchCursor(o, ['GiZScore', 'NNeighbors'])][:5]
o = P(D) + '/hs_inv0.shp'
S.HotSpots(bs, 'V', o, 'INVERSE_DISTANCE', 'EUCLIDEAN_DISTANCE', 'NONE', None)
out['hot:inv:none'] = [list(r) for r in arcpy.da.SearchCursor(o, ['GiZScore', 'NNeighbors'])][:5]
o = P(D) + '/lisa_inv0.shp'
S.ClustersOutliers(bs, 'V', o, 'INVERSE_DISTANCE', 'EUCLIDEAN_DISTANCE', 'ROW', 0, None, 'NO_FDR', 0)
out['lisa:inv:0'] = [list(r) for r in arcpy.da.SearchCursor(o, ['LMiIndex', 'LMiZScore', 'NNeighbors'])][:5]
print(out)
json.dump(out, open(P(D) + '/out9.json', 'w'))
