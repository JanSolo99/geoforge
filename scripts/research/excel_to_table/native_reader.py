"""Read native outputs using public ArcGIS; run after creating the saved /tmp config."""
import json
with open(P('/tmp/unigis-excel-native-config.json')) as f:
 config = json.load(f)
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/excel_to_table/results.json')) as f:
 oracle = json.load(f)
results = {}
for name in config['names']:
 path = P(config['folder'] + '/output.gdb/' + name)
 expected = oracle[name + '_gdb']
 fields = arcpy.ListFields(path)
 actual = [dict(name=f.name, alias=f.aliasName, type=f.type, length=f.length, precision=f.precision, scale=f.scale) for f in fields]
 rows = [list(row) for row in arcpy.da.SearchCursor(path, [f.name for f in fields if f.type != 'OID'])]
 # Field order, OID metadata, aliases, exact types/widths and values all matter.
 assert actual == expected['fields'], (name, actual, expected['fields'])
 assert rows == expected['rows'], (name, rows, expected['rows'])
 results[name] = dict(status='matched', rows=len(rows))
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/excel_to_table/native_reader_results.json'), 'w') as f:
 json.dump(results,f,indent=2)
print('ArcGIS native reader matched',len(results),'tables')
