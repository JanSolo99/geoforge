"""Read saved ArcGIS/native TIFFs via GDAL file IO; no GDAL GP/statistics calls."""
import json
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from osgeo import gdal

gdal.UseExceptions()
root=Path(__file__).resolve().parents[3]
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
failures=[]
with tempfile.TemporaryDirectory(prefix='unigis-line-compare-') as folder:
 for name,info in manifest['cases'].items():
  if not name.startswith('ls_'):continue
  output=Path(folder)/(name+'.tif')
  args=[str(root/'target/debug/ug'),'run',info['run']['tool']]
  args += [f'{k}={output if v=="<out>" else v}' for k,v in info['run']['params'].items()]
  subprocess.run(args,cwd=root,check=True,capture_output=True)
  expected=gdal.Open(str(root/'testdata/arcgis-reference'/(name+'.tif')))
  actual=gdal.Open(str(output))
  want=expected.ReadAsArray();got=actual.ReadAsArray()
  wn=expected.GetRasterBand(1).GetNoDataValue();gn=actual.GetRasterBand(1).GetNoDataValue()
  missing=want==wn;ours_missing=got==gn
  equal=(missing&ours_missing)|(~missing&~ours_missing&(np.abs(got.astype(float)-want.astype(float))<=1e-5*np.maximum(1,np.abs(want.astype(float)))))
  bad=np.argwhere(~equal)
  differences=[]
  if actual.GetGeoTransform()!=expected.GetGeoTransform():differences.append('grid differs')
  if actual.GetRasterBand(1).DataType!=expected.GetRasterBand(1).DataType:differences.append('type differs')
  if wn!=gn:differences.append('NoData differs')
  if len(bad):differences.append(f'{len(bad)} cells differ: '+str([(int(r),int(c),float(got[r,c]),float(want[r,c])) for r,c in bad[:4]]))
  print(name, differences or 'PASS')
  if differences:failures.append(name)
raise SystemExit(bool(failures))
