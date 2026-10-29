"""Compare native ppn_* tables with saved ArcGIS outputs; GDAL reads files only."""
import json
import math
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

def table(path):
    result = subprocess.run(['ogrinfo', '-json', '-features', '-al', str(path)],
                            check=True, capture_output=True, text=True)
    layer = json.loads(result.stdout)['layers'][0]
    schema = [(f['name'], f['type'], f.get('width'), f.get('precision')) for f in layer['fields']]
    return schema, [list(f['properties'].values()) for f in layer['features']]

def equal(a, b):
    if isinstance(a, (int, float)) and isinstance(b, (int, float)):
        return math.isclose(a, b, rel_tol=1e-11, abs_tol=1e-6)
    return a == b

def main():
    cases = json.loads((ROOT / 'testdata/arcgis-reference/manifest.json').read_text())['cases']
    count = rows = 0
    with tempfile.TemporaryDirectory(prefix='unigis-ppn-compare-') as temporary:
        for name, case in cases.items():
            if not name.startswith('ppn_'):
                continue
            output = Path(temporary) / (name + '.dbf')
            params = case['run']['params']
            args = [f'{k}={output if k == "output" else v}' for k, v in params.items()]
            result = subprocess.run([str(ROOT / 'target/debug/ug'), 'run',
                                     'pairwise_polygon_neighbors', *args], cwd=ROOT,
                                    capture_output=True, text=True)
            if case['status'] == 'failed':
                assert result.returncode != 0, name
                print(name, 'refused')
            else:
                assert result.returncode == 0, (name, result.stderr)
                a, b = table(output), table(ROOT / 'testdata/arcgis-reference' / (name + '.dbf'))
                assert a[0] == b[0], (name, 'schema', a[0], b[0])
                assert len(a[1]) == len(b[1]), (name, len(a[1]), len(b[1]))
                for i, (ra, rb) in enumerate(zip(a[1], b[1])):
                    assert len(ra) == len(rb) and all(equal(x,y) for x,y in zip(ra,rb)), (name,i,ra,rb)
                rows += len(a[1])
                print(name, len(a[1]), 'rows PASS')
            count += 1
    print(f'{count} cases / {rows} rows PASS')

if __name__ == '__main__':
    main()
