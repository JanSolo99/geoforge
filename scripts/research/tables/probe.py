import os
import arcpy, json, random
arcpy.env.overwriteOutput = True
D = '/tmp/claude-1000/tb'
sr = arcpy.SpatialReference(32632)
random.seed(3)
fc = arcpy.management.CreateFeatureclass(P(D), 'src.shp', 'POINT', spatial_reference=sr)
for f, t, ln in [('NAME', 'TEXT', 20), ('POP', 'LONG', None), ('AREA', 'DOUBLE', None), ('CODE', 'TEXT', 4)]:
    arcpy.management.AddField(fc, f, t, field_length=ln) if ln else arcpy.management.AddField(fc, f, t)
rows = [('alpha', 10, 1.5, 'A1'), ('beta', 20, 2.5, 'B2'), ('gamma', 30, 3.5, 'A1'), ('delta', 0, 4.5, 'C3'), ('eps', 50, 0.0, 'B2')]
with arcpy.da.InsertCursor(fc, ['SHAPE@XY', 'NAME', 'POP', 'AREA', 'CODE']) as c:
    for i, r in enumerate(rows): c.insertRow([(500000 + i * 10, 5000000), *r])
src = P(D) + '/src.shp'
# a join table
arcpy.management.CreateTable(P(D), 'jt.dbf')
jt = P(D) + '/jt.dbf'
for f, t, ln in [('CODE', 'TEXT', 4), ('LABEL', 'TEXT', 30), ('WEIGHT', 'DOUBLE', None), ('RANK', 'LONG', None)]:
    arcpy.management.AddField(jt, f, t, field_length=ln) if ln else arcpy.management.AddField(jt, f, t)
with arcpy.da.InsertCursor(jt, ['CODE', 'LABEL', 'WEIGHT', 'RANK']) as c:
    for r in [('A1', 'first', 0.5, 1), ('B2', 'second', 1.5, 2), ('Z9', 'unused', 9.9, 9)]: c.insertRow(r)
out = {'src': rows}
def dump(path):
    d = arcpy.Describe(path)
    fields = [(f.name, f.type, f.length, f.precision, f.scale, f.aliasName, f.isNullable) for f in d.fields]
    names = [f.name for f in d.fields if f.type != 'Geometry']
    rws = []
    with arcpy.da.SearchCursor(path, names) as cur:
        for r in cur: rws.append(list(r))
    return {'fields': fields, 'rows': rws, 'type': d.datasetType}
cnt = [0]
def go(key, fn, copy=None):
    cnt[0] += 1
    o = P(D) + '/o%d.%s' % (cnt[0], 'dbf')
    try:
        if copy:
            o = P(D) + '/c%d.shp' % cnt[0]
            arcpy.management.CopyFeatures(copy, o); fn(o)
        else:
            fn(o)
        out[key] = dump(o); print('ok', key, [(f[0], f[1], f[2]) for f in out[key]['fields']])
    except Exception as e:
        print('FAIL', key, str(e)[:150])
go('copyrows', lambda o: arcpy.management.CopyRows(src, o))
go('copyrows_jt', lambda o: arcpy.management.CopyRows(jt, o))
go('t2t', lambda o: arcpy.conversion.TableToTable(src, P(D), os.path.basename(o) if False else 'o%d.dbf' % cnt[0]) if False else arcpy.conversion.TableToTable(src, P(D), 'o%d.dbf' % cnt[0]))
go('t2t_where', lambda o: arcpy.conversion.TableToTable(src, P(D), 'o%d.dbf' % cnt[0], "POP > 15"))
go('join_all', lambda o: arcpy.management.JoinField(o, 'CODE', jt, 'CODE'), copy=src)
go('join_some', lambda o: arcpy.management.JoinField(o, 'CODE', jt, 'CODE', ['LABEL', 'RANK']), copy=src)
go('join_dupname', lambda o: arcpy.management.JoinField(o, 'NAME', jt, 'LABEL', ['CODE']), copy=src)
go('alter_name', lambda o: arcpy.management.AlterField(o, 'POP', 'POPULATION', 'Population count'), copy=src)
go('alter_alias', lambda o: arcpy.management.AlterField(o, 'AREA', new_field_alias='Area m2'), copy=src)
go('addfields', lambda o: arcpy.management.AddFields(o, [['F1', 'LONG'], ['F2', 'TEXT', 'Alias2', 12], ['F3', 'DOUBLE', 'Alias3', None, 7.5], ['F4', 'SHORT'], ['F5', 'FLOAT'], ['F6', 'DATE']]), copy=src)
go('delrows', lambda o: arcpy.management.DeleteRows(o), copy=src)
json.dump(out, open(P(D) + '/tb1.json', 'w'), default=str)
