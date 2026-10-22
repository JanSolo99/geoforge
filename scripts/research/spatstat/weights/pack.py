"""Packs the ArcGIS probe outputs (probe_p3..p9.py, run through tools/arcgis-bridge/run_code.py
into a scratch directory) into testdata/arcgis-reference/spatstat_weights.json.

    python3 pack.py <scratch>/sw2 <repo>/testdata/arcgis-reference/spatstat_weights.json
"""
import json, sys, os
sys.path.insert(0, os.path.dirname(__file__))
exec(open(os.path.join(os.path.dirname(__file__), 'dec.inc')).read())
src, dst = sys.argv[1], sys.argv[2]
sys.path.insert(0, os.path.dirname(__file__))
from harness_cases import CASES
out = {'swm': {}, 'moran': {}, 'hot': {}, 'lisa': {}, 'moran_default_band': {}}
for k, v in CASES.items():
    d = dec(os.path.join(src, k + '.swm'))
    hdr = ';'.join(x for x in d['header'].split(';') if not x.startswith('INPUTFC'))
    out['swm'][k] = {'input': v[0], 'uid': v[2] if len(v) > 2 else 'UID', 'args': v[1],
                     'header': hdr, 'rowstd': d['rowstd'], 'count': d['n'], 'rows': d['rows']}
out['tables'] = json.load(open(os.path.join(src, 'out.json')))['tables']
for k in out['swm']:
    out['swm'][k]['input'] = out['swm'][k]['input'].replace('../swm/', '')
o8 = json.load(open(os.path.join(src, 'out8.json')))
o9 = json.load(open(os.path.join(src, 'out9.json')))
out['moran'] = {k: [float(x) for x in v] for k, v in o8['moran'].items()}
out['moran'].update({k: [float(x) for x in v] for k, v in o9.items() if k.count(':') == 2 and not k.startswith(('hot', 'lisa'))})
out['hot'] = {k: v['rows'] for k, v in o8['hot'].items()}
out['hot_fields'] = {k: [f[0] for f in v['fields']] for k, v in o8['hot'].items()}
out['lisa'] = {k: v['rows'] for k, v in o8['lisa'].items() if 'p99' not in k}
out['lisa_fields'] = [f[:3] for f in o8['lisa']['big:inv:ROW']['fields']]
out['hot']['big:fix:none'] = o9['hot:fix:none']
out['lisa']['big:inv:0'] = o9['lisa:inv:0']
json.dump(out, open(dst, 'w'), separators=(',', ':'))
print(len(out['swm']), 'weights files', os.path.getsize(dst), 'bytes')
