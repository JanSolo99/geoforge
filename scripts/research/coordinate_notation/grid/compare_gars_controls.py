"""Check native grid-to-GARS text against saved ArcGIS input families and holdouts."""
import csv
import json
import subprocess
import sys
import tempfile
from pathlib import Path

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE.parent))
from compare import ROOT, read


def main():
    controls=json.loads((HERE/'gars_holdout_results.json').read_text())
    old=json.loads((HERE/'probe_boundaries_results.txt').read_text().splitlines()[0].split(' ',1)[1])
    with tempfile.TemporaryDirectory(prefix='unigis-gars-controls-') as directory:
        folder=Path(directory)
        source=folder/'old.csv'
        with source.open('w',newline='') as f:
            w=csv.writer(f);w.writerow(['ID','COORD']);w.writerows((i,code) for i,(code,_) in enumerate(old,1))
        batches=[('old GARS holdout','GARS',source,[value for _,value in old])]
        for batch in controls:
            fmt=batch['format']
            batches.append((fmt+' independent',fmt,ROOT/'testdata/geoproc'/f'gars_boundary_{fmt.lower()}.dbf',
                            [row['output'] for row in batch['rows']]))
        count=0
        for label,fmt,source,expected in batches:
            output=folder/'out.shp'
            subprocess.run([str(ROOT/'target/debug/ug'),'run','convert_coordinate_notation',
                f'input={source}',f'output={output}','x_field=COORD',f'input_format={fmt}','output_format=GARS'],
                check=True,capture_output=True)
            actual=[row['properties']['GARS'] or '' for row in read(output)['features']]
            assert len(actual)==len(expected),(label,len(actual),len(expected))
            bad=[(i,a,b) for i,(a,b) in enumerate(zip(actual,expected),1) if a!=b]
            assert not bad,(label,bad[:10])
            print(label,len(actual),'exact PASS');count+=len(actual)
        print(count,'native grid-to-GARS controls match ArcGIS exactly')


if __name__=='__main__':main()
