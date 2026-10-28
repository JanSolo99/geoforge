"""ArcGIS supplies behavior; GDAL reads saved TIFF samples/tags/caches only."""
import json
from pathlib import Path
import subprocess
import tempfile
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
root=Path(__file__).resolve().parents[4]
manifest=json.loads((root/'testdata/arcgis-reference/manifest.json').read_text())
failures=[];count=0
with tempfile.TemporaryDirectory(prefix='unigis-multicache-') as folder:
    for name,info in manifest['cases'].items():
        if not name.startswith(('cprmulti_','ummulti_')):continue
        count+=1
        output=Path(folder)/(name+'.tif')
        args=[str(root/'target/debug/ug'),'run',info['run']['tool']]
        args += [f'{k}={output if v=="<out>" else v}' for k,v in info['run']['params'].items()]
        subprocess.run(args,cwd=root,check=True,capture_output=True)
        reference=root/'testdata/arcgis-reference'/(name+'.tif')
        expected=gdal.Open(str(reference));actual=gdal.Open(str(output));differences=[]
        if actual.GetGeoTransform()!=expected.GetGeoTransform():differences.append('grid differs')
        if actual.RasterCount!=expected.RasterCount: differences.append(f'bands {actual.RasterCount} vs {expected.RasterCount}')
        if Path(str(output)+'.aux.xml').exists()!=Path(str(reference)+'.aux.xml').exists():differences.append('PAM placement differs')
        for index in range(1,min(actual.RasterCount,expected.RasterCount)+1):
            a=actual.GetRasterBand(index);e=expected.GetRasterBand(index)
            if a.DataType!=e.DataType:differences.append(f'band{index} type differs')
            an=a.GetNoDataValue();en=e.GetNoDataValue()
            if an!=en:differences.append(f'band{index} NoData {an} vs {en}')
            av=a.ReadAsArray();ev=e.ReadAsArray()
            amask=av==an;emask=ev==en
            if not np.all((amask&emask)|(~amask&~emask&(av==ev))):differences.append(f'band{index} pixels differ')
            em=e.GetMetadata();am=a.GetMetadata()
            for key,tolerance in [('COUNT',0),('MINIMUM',1e-5),('MAXIMUM',1e-5),('MEAN',1e-4),('STDDEV',1e-3)]:
                key='STATISTICS_'+key
                if key in em and (key not in am or abs(float(am[key])-float(em[key]))>tolerance*max(1,abs(float(em[key])))):differences.append(f'band{index} {key} differs')
        print(name,differences or 'PASS')
        if differences:failures.append(name)
print('Checked',count,'FAILED',len(failures),failures)
if not count:raise SystemExit('No reference cases found')
raise SystemExit(bool(failures))
