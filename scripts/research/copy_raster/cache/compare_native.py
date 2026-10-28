"""Compare saved ArcGIS outputs using GDAL only to read file contents/metadata."""
import json
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from osgeo import gdal

gdal.UseExceptions()
root=Path(__file__).resolve().parents[4]
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
failures=[]
with tempfile.TemporaryDirectory(prefix='unigis-copy-nd-compare-') as folder:
 for name,info in manifest['cases'].items():
  if not name.startswith(('cprcache_',)):continue
  output=Path(folder)/(name+'.tif')
  args=[str(root/'target/debug/ug'),'run',info['run']['tool']]
  args += [f'{k}={output if v=="<out>" else v}' for k,v in info['run']['params'].items()]
  subprocess.run(args,cwd=root,check=True,capture_output=True)
  expected=gdal.Open(str(root/'testdata/arcgis-reference'/(name+'.tif')))
  actual=gdal.Open(str(output))
  want=expected.ReadAsArray();got=actual.ReadAsArray()
  eb=expected.GetRasterBand(1);ab=actual.GetRasterBand(1)
  wn=eb.GetNoDataValue();gn=ab.GetNoDataValue()
  missing=want==wn;ours_missing=got==gn
  equal=(missing&ours_missing)|(~missing&~ours_missing&(got==want))
  differences=[]
  if actual.GetGeoTransform()!=expected.GetGeoTransform():differences.append('grid differs')
  if ab.DataType!=eb.DataType:differences.append('type differs')
  if wn!=gn:differences.append(f'NoData {gn} vs {wn}')
  bad=np.argwhere(~equal)
  if len(bad):differences.append(f'{len(bad)} cells differ: '+str([(int(r),int(c),float(got[r,c]),float(want[r,c])) for r,c in bad[:2]]))
  em=eb.GetMetadata();am=ab.GetMetadata()
  if ('STATISTICS_MEAN' in em)!=('STATISTICS_MEAN' in am):differences.append('statistics presence differs')
  if 'STATISTICS_MEAN' in em:
   for key,tol in [('COUNT',0),('MINIMUM',1e-5),('MAXIMUM',1e-5),('MEAN',1e-4),('STDDEV',1e-3)]:
    k='STATISTICS_'+key
    if k not in am or abs(float(am[k])-float(em[k]))>tol*max(1,abs(float(em[k]))):differences.append(k+' differs')
  print(name,differences or 'PASS')
  if differences:failures.append(name)
print('FAILED',len(failures),failures)
raise SystemExit(bool(failures))
