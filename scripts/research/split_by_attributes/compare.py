"""Saved ArcGIS workspace names/schema/rows; geometry is checked by scripts/check."""
import json
import math
import subprocess
import tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
def read(path):
    result=subprocess.run(['ogrinfo','-json','-features','-al',str(path)],check=True,capture_output=True,text=True)
    layer=json.loads(result.stdout)['layers'][0]
    return ([(f['name'],f['type'],f.get('width'),f.get('precision')) for f in layer['fields']],
            [list(f['properties'].values()) for f in layer['features']])
def equal(a,b):
    if isinstance(a,(int,float)) and isinstance(b,(int,float)):
        return math.isclose(a,b,abs_tol=1e-6,rel_tol=1e-11)
    return a==b

def main():
    manifest=json.loads((ROOT/'testdata/arcgis-reference/manifest.json').read_text())['cases']
    cases=datasets=rows=0
    with tempfile.TemporaryDirectory(prefix='unigis-sba-compare-') as temporary:
        for name,case in manifest.items():
            if not name.startswith('sba_'):continue
            folder=Path(temporary)/name;folder.mkdir()
            args=[f'{k}={folder if v=="<out>" else v}' for k,v in case['run']['params'].items()]
            result=subprocess.run([str(ROOT/'target/debug/ug'),'run','split_by_attributes',*args],cwd=ROOT,capture_output=True,text=True)
            if case['status']=='failed':assert result.returncode!=0,(name,result.stdout)
            else:
                assert result.returncode==0,(name,result.stderr)
                found={p.name for p in folder.iterdir() if p.suffix=='.shp' or p.suffix=='.dbf' and not p.with_suffix('.shp').exists()}
                assert found==set(case['outputs']),(name,found,set(case['outputs']))
                for dataset in sorted(found):
                    a,b=read(folder/dataset),read(ROOT/'testdata/arcgis-reference'/name/dataset)
                    assert a[0]==b[0],(name,dataset,a[0],b[0])
                    assert len(a[1])==len(b[1]),(name,dataset)
                    for i,(ra,rb) in enumerate(zip(a[1],b[1])):
                        assert len(ra)==len(rb) and all(equal(x,y) for x,y in zip(ra,rb)),(name,dataset,i,ra,rb)
                    datasets+=1;rows+=len(a[1])
            cases+=1;print(name,'PASS')
    print(f'{cases} cases, {datasets} datasets, {rows} rows PASS (geometry pending full gate)')
if __name__=='__main__':main()
