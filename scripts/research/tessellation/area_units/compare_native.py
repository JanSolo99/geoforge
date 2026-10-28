"""Compare ArcGIS-generated tessellation outputs; OGR reads files only."""
from pathlib import Path
import json
import math
import subprocess
import tempfile
from osgeo import ogr
ogr.UseExceptions()
root=Path(__file__).resolve().parents[4]
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
failures=[];count=0
with tempfile.TemporaryDirectory(prefix='unigis-tess-area-') as folder:
    for name,case in manifest['cases'].items():
        if not name.startswith('tess_area_'):continue
        count+=1;output=Path(folder)/(name+'.shp')
        args=[str(root/'target/debug/ug'),'run',case['run']['tool']]
        args += [f'{k}={output if v=="<out>" else v}' for k,v in case['run']['params'].items()]
        subprocess.run(args,cwd=root,check=True,capture_output=True)
        expected=ogr.Open(str(root/'testdata/arcgis-reference'/(name+'.shp')))
        actual=ogr.Open(str(output))
        a=actual.GetLayer(0);e=expected.GetLayer(0);diff=[]
        actual_prj=output.with_suffix('.prj');expected_prj=(root/'testdata/arcgis-reference'/(name+'.prj'))
        if actual_prj.exists()!=expected_prj.exists() or (actual_prj.exists() and actual_prj.read_bytes()!=expected_prj.read_bytes()):diff.append('exact PRJ differs')
        actual_sr=a.GetSpatialRef();expected_sr=e.GetSpatialRef()
        if (actual_sr is None)!=(expected_sr is None) or (actual_sr is not None and expected_sr is not None and not actual_sr.IsSame(expected_sr)):diff.append('CRS differs')
        if a.GetFeatureCount()!=e.GetFeatureCount():diff.append('count differs')
        for index,(got,want) in enumerate(zip(a,e)):
            if [got.GetField('Id'),got.GetField('GRID_ID')]!=[want.GetField('Id'),want.GetField('GRID_ID')]:diff.append(f'cell{index} fields/order differ')
            g=json.loads(got.GetGeometryRef().ExportToJson())['coordinates'][0]
            w=json.loads(want.GetGeometryRef().ExportToJson())['coordinates'][0]
            if len(g)!=len(w) or any(math.hypot(p[0]-q[0],p[1]-q[1])>0.001 for p,q in zip(g,w)):diff.append(f'cell{index} ordered ring differs')
        print(name,diff or 'PASS')
        if diff:failures.append(name)
print('Checked',count,'FAILED',len(failures),failures)
if not count:raise SystemExit('No reference cases found')
raise SystemExit(bool(failures))
