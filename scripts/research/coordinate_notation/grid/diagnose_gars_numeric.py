"""Verify inferred numeric model; report the unchanged angular path's boundary gap."""
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE.parent))
from compare import ROOT,read
from gars_model import encode_grid


def main():
    rows=json.loads((HERE/'gars_numeric_results.json').read_text())
    bad=[r for r in rows if encode_grid((float(format(r['input_x'],'.15g')),
                                       float(format(r['input_y'],'.15g'))))!=r['numeric_output']]
    assert not bad,bad[:3]
    print(len(rows),'numeric CSV controls fit 15-significant-digit + angle roundtrip + global ticks')
    with tempfile.TemporaryDirectory(prefix='unigis-gars-angular-diagnostic-') as directory:
        folder=Path(directory)
        source=folder/'numeric.csv'
        with source.open('w',newline='') as f:
            writer=csv.writer(f);writer.writerow(['ID','LON','LAT'])
            writer.writerows((r['id'],format(r['input_x'],'.17g'),format(r['input_y'],'.17g')) for r in rows)
        output=folder/'out.shp'
        subprocess.run([str(ROOT/'target/debug/ug'),'run','convert_coordinate_notation',
                        f'input={source}',f'output={output}','x_field=LON','y_field=LAT','output_format=GARS'],
                       check=True,capture_output=True)
        features=read(output)['features']
        assert len(features)==len(rows)
        differences=[(r['id'],r['code'],r['variant'],f['properties']['GARS'],r['numeric_output'])
                     for r,f in zip(rows,features) if f['properties']['GARS']!=r['numeric_output']]
        print('Unchanged angular path:',len(differences),'boundary differences remain')
        print('Examples:',differences[:4])


if __name__=='__main__':main()
