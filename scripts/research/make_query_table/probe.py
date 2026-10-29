"""Public ArcGIS query-view controls; no persistent source edits."""
import json
source = P('/mnt/media/ArcGIS_Linux/testdata/geoproc/globalids_base.gdb/points')
results = {}
for name, key, fields, where in [
    ('default', 'ADD_VIRTUAL_KEY_FIELD', '', ''),
    ('projected', 'NO_KEY_FIELD', [['points.OBJECTID', 'ID']], 'points.OBJECTID <= 2'),
    ('empty', 'USE_KEY_FIELDS', '', 'points.OBJECTID < 0'),
]:
    try:
        out = arcpy.management.MakeQueryTable(source, 'query_probe_' + name, key, '', fields, where).getOutput(0)
        d = arcpy.Describe(out)
        results[name] = dict(status='success', output=str(out), data_type=d.dataType,
                             catalog_path=getattr(d, 'catalogPath', None),
                             count=int(arcpy.management.GetCount(out)[0]),
                             fields=[dict(name=f.name, alias=f.aliasName, type=f.type) for f in arcpy.ListFields(out)])
    except Exception as e:
        results[name] = dict(status='error', error=str(e), messages=arcpy.GetMessages())
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/make_query_table/results.json'), 'w') as f:
    json.dump(results, f, indent=2)
print(json.dumps(results, indent=2))
