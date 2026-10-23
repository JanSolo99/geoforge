import arcpy, json, random
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/sw2'
sr = arcpy.SpatialReference(32632)
random.seed(4)
locs = [(500000 + random.randint(0, 20) * 10.0, 5000000 + random.randint(0, 10) * 10.0) for _ in range(15)]
pts = [random.choice(locs) for _ in range(60)] + [(500000.0004, 5000000.0003), (500000.0, 5000000.0), (500100.00049, 5000050.0)]
fc = arcpy.management.CreateFeatureclass(P(D), 'ev.shp', 'POINT', spatial_reference=sr)
arcpy.management.AddField(fc, 'A', 'LONG')
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'A']) as c:
    for i, p in enumerate(pts): c.insertRow([p, i])
out = {'pts': pts}
o = P(D) + '/ev_out.shp'
r = arcpy.stats.CollectEvents(P(D) + '/ev.shp', o)
d = arcpy.Describe(o)
out['fields'] = [(f.name, f.type, f.length, f.precision) for f in d.fields]
out['rows'] = [list(x) for x in arcpy.da.SearchCursor(o, ['SHAPE@X', 'SHAPE@Y'] + [f.name for f in d.fields if f.type not in ('Geometry',)])]
out['msg'] = r.getMessages(); out['outputs'] = [str(r.getOutput(i)) for i in range(r.outputCount)]
o2 = P(D) + '/evg.gdb'
if not arcpy.Exists(o2): arcpy.management.CreateFileGDB(P(D), 'evg.gdb')
arcpy.stats.CollectEvents(P(D) + '/ev.shp', o2 + '/out')
out['gdb_fields'] = [(f.name, f.type, f.length) for f in arcpy.Describe(o2 + '/out').fields]
print(out['fields'], out['rows'][:5], out['msg'], out['outputs'], out['gdb_fields'])
json.dump(out, open(P(D) + '/ev.json', 'w'))
