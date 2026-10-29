"""Black-box ArcGIS controls, disposable outputs under /tmp."""
import json, os
base = P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
folder = P('/tmp/unigis-excel-probe')
os.makedirs(folder, exist_ok=True)
gdb = folder + '/output.gdb'
if not arcpy.Exists(gdb): arcpy.management.CreateFileGDB(folder, 'output.gdb')
results = {}
for name, ext, sheet, header, cells in [
 ('default', 'xlsx', '', 1, ''), ('no_header', 'xlsx', 'Data', 0, 'A1:C3'),
 ('range', 'xlsx', 'Data', 1, 'B2:D3'), ('names', 'xlsx', 'Names', 1, ''),
 ('offset', 'xlsx', 'Offset', 3, ''), ('dates', 'xlsx', 'Dates', 1, ''),
 ('named', 'xlsx', 'Picked', 1, ''), ('xls', 'xls', '', 1, ''),
 ('empty', 'xlsx', 'Empty', 1, ''), ('bad_sheet', 'xlsx', 'Missing', 1, ''),
 ('reverse', 'xlsx', 'Data', 1, 'D3:B2'), ('beyond', 'xlsx', 'Data', 1, 'A3:C7'),
 ('header_outside', 'xlsx', 'Data', 8, ''),
]:
 for kind in ['dbf', 'gdb']:
  key = name + '_' + kind
  output = gdb + '/' + name if kind == 'gdb' else folder + '/' + name + '.dbf'
  try:
   arcpy.conversion.ExcelToTable(base + '/excel_input.' + ext, output, sheet, header, cells)
   fields = arcpy.ListFields(output)
   names = [f.name for f in fields if f.type != 'OID']
   rows = [list(r) for r in arcpy.da.SearchCursor(output, names)]
   results[key] = dict(status='success', fields=[dict(name=f.name, alias=f.aliasName, type=f.type, length=f.length, precision=f.precision, scale=f.scale) for f in fields], rows=rows, messages=arcpy.GetMessages())
  except Exception as e:
   results[key] = dict(status='error', error=str(e), messages=arcpy.GetMessages())
with open(P('/mnt/media/ArcGIS_Linux/scripts/research/excel_to_table/results.json'), 'w') as f:
 json.dump(results, f, indent=2, default=str)
print('Saved', len(results), 'Excel controls')
