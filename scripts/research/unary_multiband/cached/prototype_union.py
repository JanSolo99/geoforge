"""Test a union of stored ranges using native single-band planning only.

GDAL supplies file slicing/read-back, never geoprocessing behavior.
"""
from pathlib import Path
import json,subprocess
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
folder=Path('/tmp/unigis-cached-int-multiband');profiles=json.loads((folder/'profiles.json').read_text())
rows=[]
for name in profiles:
 source=folder/(name+'.tif');dataset=gdal.Open(str(source))
 fields=[dataset.GetRasterBand(i).GetMetadata() for i in [1,2]]
 lo=min(float(m['STATISTICS_MINIMUM']) for m in fields);hi=max(float(m['STATISTICS_MAXIMUM']) for m in fields)
 for index in [1,2]:
  single=folder/(name+f'_band{index}.tif')
  if single.exists():raise SystemExit(f'Already exists:{single}')
  translated=gdal.Translate(str(single),dataset,bandList=[index]);translated=None
  Path(str(single)+'.aux.xml').write_text(f'<PAMDataset><PAMRasterBand band="1"><Metadata><MDI key="STATISTICS_MINIMUM">{lo}</MDI><MDI key="STATISTICS_MAXIMUM">{hi}</MDI></Metadata></PAMRasterBand></PAMDataset>')
  for op in ['Abs','Negate','Square']:
   output=folder/(name+f'_band{index}_{op}_native.tif')
   subprocess.run(['/mnt/media/ArcGIS_Linux/target/debug/ug','run',op.lower(),f'input={single}',f'output={output}'],check=True,capture_output=True)
   native=gdal.Open(str(output));reference=gdal.Open(str(folder/(name+'_'+op+'.tif')))
   a=native.GetRasterBand(1);e=reference.GetRasterBand(index);diff=[]
   if a.DataType!=e.DataType:diff.append(f'type {gdal.GetDataTypeName(a.DataType)} vs {gdal.GetDataTypeName(e.DataType)}')
   if a.GetNoDataValue()!=e.GetNoDataValue():diff.append(f'ND {a.GetNoDataValue()} vs {e.GetNoDataValue()}')
   av=a.ReadAsArray();ev=e.ReadAsArray();an=a.GetNoDataValue();en=e.GetNoDataValue()
   if not np.all(((av==an)&(ev==en))|((av!=an)&(ev!=en)&(av==ev))):diff.append('samples/mask')
   am=a.GetMetadata();em=e.GetMetadata()
   for key,tol in [('COUNT',0),('MINIMUM',1e-5),('MAXIMUM',1e-5),('MEAN',1e-4),('STDDEV',1e-3)]:
    key='STATISTICS_'+key
    if key in em and (key not in am or abs(float(am[key])-float(em[key]))>tol*max(1,abs(float(em[key])))):diff.append(key)
   row={'case':name,'op':op,'band':index,'global_range':[lo,hi],'differences':diff};rows.append(row)
   print(name,op,index,diff or 'PASS')
   native=None;reference=None
 dataset=None
(folder/'union_prototype.json').write_text(json.dumps(rows,indent=2)+'\n')
print('Checked',len(rows),'FAILED',sum(bool(row['differences']) for row in rows))
raise SystemExit(any(row['differences'] for row in rows))
