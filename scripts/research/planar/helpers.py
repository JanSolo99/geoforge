import arcpy, json, os
arcpy.env.overwriteOutput = True
PG_D = P('/tmp/claude-1000/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/scratchpad/pg')
PG_SR = arcpy.SpatialReference(32632)
PG_X0, PG_Y0 = 500000, 5000000
def pg_poly(parts):
    return arcpy.Polygon(arcpy.Array([arcpy.Array([arcpy.Point(PG_X0 + x, PG_Y0 + y) for x, y in r]) for r in parts]), PG_SR)
def pg_line(parts):
    return arcpy.Polyline(arcpy.Array([arcpy.Array([arcpy.Point(PG_X0 + x, PG_Y0 + y) for x, y in r]) for r in parts]), PG_SR)
def pg_pts(rows):
    return [(arcpy.PointGeometry(arcpy.Point(PG_X0+x, PG_Y0+y), PG_SR), *a) for (x, y), *a in rows]
def pg_mk(name, kind, rows, fields=[('N','LONG')]):
    fc = arcpy.management.CreateFeatureclass(PG_D, name + '.shp', kind, spatial_reference=PG_SR)
    for f, t in fields: arcpy.management.AddField(fc, f, t)
    with arcpy.da.InsertCursor(fc, ['SHAPE@'] + [f for f, t in fields]) as c:
        for g, *a in rows: c.insertRow([g] + a)
    return PG_D + '/' + name + '.shp'
def pg_export(path):
    d = arcpy.Describe(path)
    names = [f.name for f in d.fields if f.type != 'Geometry']
    rows = []
    with arcpy.da.SearchCursor(path, names + ['SHAPE@']) as cur:
        for r in cur:
            g = r[-1]
            parts = []
            if g is not None:
                for part in g:
                    parts.append([[round(p.X - PG_X0, 6), round(p.Y - PG_Y0, 6)] if p else None for p in part])
            rows.append([list(r[:-1]), parts])
    return {'fields': [(f.name, f.type, f.length, f.precision, f.scale) for f in d.fields], 'rows': rows}
PG_OUT = {}
def pg_probe(key, fn, inp):
    o = PG_D + '/out_%s.shp' % key
    try:
        fn(inp, o)
        PG_OUT[key] = {'in': pg_export(inp), 'out': pg_export(o)}
        print('ok', key, len(PG_OUT[key]['out']['rows']))
    except Exception as e:
        print('FAIL', key, str(e)[:300])
def pg_save(name):
    json.dump(PG_OUT, open(PG_D + '/' + name + '.json', 'w'))
def pg_show(keys=None):
    for k in keys or list(PG_OUT):
        print(k, [f[:2] for f in PG_OUT[k]['out']['fields']][2:])
        for a, p in PG_OUT[k]['out']['rows']: print('  ', a, p)
P2L = lambda i, o: arcpy.management.PolygonToLine(i, o, 'IDENTIFY_NEIGHBORS')
FTP = lambda i, o: arcpy.management.FeatureToPolygon(i, o)
FTL = lambda i, o: arcpy.management.FeatureToLine(i, o)
