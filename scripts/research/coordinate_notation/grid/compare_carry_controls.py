"""Run native CLI against all persisted ArcGIS carry sweeps; compare exact text."""
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from compare import ROOT, read
from make_carry_inputs import coord

CONFIGS = {'east0': (0, 45, False), 'east15': (15, 45, False),
           'east151': (151, 45, False), 'west15': (-16, 45, True),
           'north45': (15, 45, False), 'south33': (15, -34, True)}


def main():
    total = 0
    with tempfile.TemporaryDirectory(prefix='unigis-carry-controls-') as directory:
        folder = Path(directory)
        for artifact in ['carry_sweep_results.json', 'carry_refine_results.json']:
            for batch in json.loads((HERE / artifact).read_text()):
                fmt, values = batch['format'], batch['values']
                source = folder / 'input.csv'
                with source.open('w', newline='') as f:
                    writer = csv.writer(f); writer.writerow(['ID', 'COORD'])
                    for index, row in enumerate(values, 1):
                        x, y, negative = CONFIGS[row['group']]
                        minute = row['minute_units']
                        if negative: minute = 600000000 - minute
                        xm, ym = (minute, 100000000) if row['axis'] == 'x' else (100000000, minute)
                        writer.writerow([index, coord(x, xm, y, ym)])
                output = folder / 'output.shp'
                subprocess.run([str(ROOT / 'target/debug/ug'), 'run', 'convert_coordinate_notation',
                                f'input={source}', 'x_field=COORD', 'input_format=GEOREF',
                                f'output_format={fmt}', f'output={output}'],
                               check=True, capture_output=True)
                features = read(output)['features']
                assert len(features) == len(values)
                field = 'DMSLatLon' if fmt == 'DMS_1' else 'DDMLatLon'
                errors = [(row['group'], row['minute_units'], feature['properties'][field], row['text'])
                          for feature, row in zip(features, values)
                          if feature['properties'][field] != row['text']]
                assert not errors, (artifact, fmt, errors[:8])
                total += len(values)
                print(artifact, fmt, len(values), 'exact PASS')
    print(total, 'ArcGIS text controls exactly match native CLI')


if __name__ == '__main__':
    main()
