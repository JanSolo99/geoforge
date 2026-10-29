"""Run saved angular cases and compare file schemas/rows using GDAL as an IO oracle."""
import argparse
import json
import math
import re
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def read(path):
    result = subprocess.run(
        ['ogrinfo', '-ro', '-al', '-json', '-features', str(path)],
        check=True, capture_output=True, text=True,
    )
    return json.loads(result.stdout)['layers'][0]


def equal(a, b):
    if isinstance(a, (float, int)) and isinstance(b, (float, int)):
        return math.isclose(a, b, rel_tol=0, abs_tol=1e-8)
    if isinstance(a, dict) and isinstance(b, dict):
        return a.keys() == b.keys() and all(equal(a[k], b[k]) for k in a)
    if isinstance(a, list) and isinstance(b, list):
        return len(a) == len(b) and all(equal(x, y) for x, y in zip(a, b))
    return a == b


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--prefix', default='ccn_input_')
    parser.add_argument('--count', type=int, default=12)
    options = parser.parse_args()
    manifest = json.loads((ROOT / 'testdata/arcgis-reference/manifest.json').read_text())
    with tempfile.TemporaryDirectory(prefix='unigis-ccn-compare-') as directory:
        count = 0
        for name, case in manifest['cases'].items():
            if not name.startswith(options.prefix):
                continue
            output = Path(directory) / f'{name}.shp'
            spec = case['run']
            params = dict(spec['params'], output=str(output))
            execution = subprocess.run(
                [str(ROOT / 'target/debug/ug'), 'run', spec['tool']]
                + [f'{key}={value}' for key, value in params.items()],
                check=True, cwd=ROOT, capture_output=True,
            )
            expected_warning = next((re.search(r'WARNING 001293:.*?records\.', message).group()
                                     for message in case.get('messages', [])
                                     if 'WARNING 001293:' in message), None)
            if expected_warning:
                messages = (execution.stdout + execution.stderr).decode()
                assert expected_warning in messages, (name, 'warning', expected_warning, messages)
            ours = read(output)
            reference = read(ROOT / 'testdata/arcgis-reference' / f'{name}.shp')
            assert ours['fields'] == reference['fields'], (name, 'schema', ours['fields'], reference['fields'])
            assert len(ours['features']) == len(reference['features']), (name, 'row count')
            differences = [(a, b) for a, b in zip(ours['features'], reference['features']) if not equal(a, b)]
            assert not differences, (name, 'rows', differences[:5])
            print(f'{name}: PASS ({len(ours["features"])} rows)')
            count += 1
        assert count == options.count, count
        print(f'{count}/{options.count} PASS; exact text/schema/null geometry, numeric tolerance 1e-8 degrees')


if __name__ == '__main__':
    main()
