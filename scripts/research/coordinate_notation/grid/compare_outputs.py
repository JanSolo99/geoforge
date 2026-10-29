"""Compare all grid-to-grid notation fields against the saved private ArcGIS probe."""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from compare import ROOT, read


def main():
    results = json.loads((HERE / 'probe_outputs_results.json').read_text())
    failures = []
    with tempfile.TemporaryDirectory(prefix='unigis-ccn-grid-output-') as directory:
        for result in results:
            source, target = result['source'], result['target']
            path = ROOT / 'testdata/geoproc' / f'grid_input_{source.lower()}.csv'
            output = Path(directory) / f'{source}_{target}.shp'
            execution = subprocess.run([str(ROOT / 'target/debug/ug'), 'run', 'convert_coordinate_notation',
                            f'input={path}', 'x_field=COORD', f'input_format={source}',
                            f'output_format={target}', f'output={output}'],
                           capture_output=True)
            assert execution.returncode == 0, execution
            actual = {f['properties']['ID']: f['properties'][target] or ''
                      for f in read(output)['features']}
            for index, expected in result['rows']:
                if actual.get(index) != expected:
                    failures.append((source, target, index, actual.get(index), expected))
            print(f'{source} -> {target}: {len(result["rows"])} rows checked')
    for failure in failures:
        print(failure)
    assert not failures, f'{len(failures)} mismatched fields'
    print('326 original GARS/GEOREF output fields match ArcGIS exactly')


if __name__ == '__main__':
    main()
