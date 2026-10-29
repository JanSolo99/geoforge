"""Create grid-input fixtures from saved ArcGIS strings plus the black-box controls."""
import ast
import csv
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def controls(path):
    tree = ast.parse(path.read_text())
    return next(ast.literal_eval(node.value) for node in tree.body
                if isinstance(node, ast.Assign)
                and any(isinstance(t, ast.Name) and t.id == 'controls' for t in node.targets))


def main():
    original = controls(HERE / 'probe.py')
    limits = controls(HERE / 'probe_limits.py')
    for fmt in ['GARS', 'GEOREF']:
        # GDAL reads strings from ArcGIS files, without computing any coordinates.
        reference = ROOT / 'testdata/arcgis-reference' / f'ccn_{fmt.lower()}.shp'
        result = subprocess.run(['ogrinfo', '-ro', '-al', '-json', '-features', str(reference)],
                                check=True, capture_output=True, text=True)
        features = json.loads(result.stdout)['layers'][0]['features']
        values = [f['properties'][fmt] for f in features] + original[fmt] + limits[fmt]
        if fmt == 'GEOREF':
            # Finest accepted minute precision on either side of GARS cell edges.
            values += ['PKAA049999999100000000', 'PKAA050000000100000000',
                       'PKAA050000001100000000', 'PKAA050000000099999999',
                       'PKAA050000000100000001']
        path = ROOT / 'testdata/geoproc' / f'grid_input_{fmt.lower()}.csv'
        with path.open('w', newline='') as f:
            writer = csv.writer(f)
            writer.writerow(['ID', 'COORD'])
            writer.writerows(enumerate(values, 1))
        print(path.name, len(values), 'rows')


if __name__ == '__main__':
    main()
