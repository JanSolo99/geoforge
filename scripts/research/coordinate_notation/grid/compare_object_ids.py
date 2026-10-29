"""Compare source ObjectID fields/values with the saved four-format ArcGIS probe."""
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from compare import ROOT, read, equal


def main():
    controls = [json.loads(line) for line in (HERE / 'probe_object_ids_results.txt').read_text().splitlines()
                if line.startswith('{')]
    root = Path('/tmp/unigis-carry-object-ids')
    sources = {'dbf': ROOT / 'testdata/geoproc/grid_carry_precision.dbf',
               'shape': root / 'points.shp', 'gdb': root / 'sources.gdb/rows',
               'csv': root / 'rows.csv'}
    with tempfile.TemporaryDirectory(prefix='unigis-carry-oid-compare-') as directory:
        for control in controls:
            kind = control['kind']
            output = Path(directory) / f'{kind}.shp'
            args = [str(ROOT / 'target/debug/ug'), 'run', 'convert_coordinate_notation',
                    f'input={sources[kind]}', f'output={output}', 'output_format=DD_NUMERIC']
            args += ['x_field=COORD', 'input_format=GEOREF'] if kind == 'dbf' else ['x_field=LON', 'y_field=LAT']
            subprocess.run(args, check=True, capture_output=True)
            layer = read(output)
            fields = [f['name'] for f in layer['fields']]
            expected_fields = [name for name in control['fields'] if name not in ('OID', 'Shape')]
            assert fields == expected_fields, (kind, fields, expected_fields)
            columns = ['DDLon', 'DDLat'] + (['ORIG_OID'] if control['hasOID'] else [])
            values = [[row['properties'][c] for c in columns] for row in layer['features'][:3]]
            assert equal(values, control['values']), (kind, values, control['values'])
            print(kind, 'exact field order and source ObjectIDs PASS')


if __name__ == '__main__':
    main()
