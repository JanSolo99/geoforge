"""Create tiny pixel-type fixtures using GDAL solely for file format conversion.

Run from the repository root. Pixel values, including integer precision boundaries,
come from matrix_inputs.json. ArcGIS, not GDAL, supplies operation reference outputs.
"""
import json
from pathlib import Path
import subprocess
import tempfile

values = json.loads(Path(__file__).with_name('matrix_inputs.json').read_text())
folder = Path('testdata/raster/composite_matrix')
folder.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory() as tmp:
    for name, (dtype, row) in [*values.items(), ('shifted', values['u8'])]:
        x = 500010 if name == 'shifted' else 500000
        source = Path(tmp) / (name + '.asc')
        source.write_text(f'ncols 3\nnrows 2\nxllcorner {x}\nyllcorner 5000000\ncellsize 10\n'
                          + ' '.join(map(str, row[:3])) + '\n' + ' '.join(map(str, row[3:])) + '\n')
        subprocess.run(['gdal_translate', '-q', '-ot', dtype, '-a_nodata', 'none', '-a_srs',
                        'EPSG:32633', str(source), str(folder / (name + '.tif'))], check=True)
