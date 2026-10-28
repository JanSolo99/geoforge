"""Create the controlled source TIFF/PAM fixtures; no GP behavior is inferred.

Pass /tmp/unigis-copy-partial-controls to prepare private oracle probe inputs.
Without arguments, recreate committed fixture pixels and cache fields.
Do not recreate committed fixtures while tests or oracle generation use them.
"""
from pathlib import Path
import json
import shutil
import sys
import xml.etree.ElementTree as ET
root=Path(__file__).resolve().parents[4]
folder=Path(sys.argv[1]) if len(sys.argv)>1 else root/'testdata/raster/copy_cache'
folder.mkdir(parents=True,exist_ok=True)
variants=json.loads(Path(__file__).with_name('inputs.json').read_text())
source=root/'testdata/raster/copy_fraction/cached_u8.tif'
for name,fields in variants.items():
    for suffix in ['', '_native'] if len(sys.argv)>1 else ['']:
        out=folder/(name+suffix+'.tif')
        shutil.copyfile(source,out)
        dataset=ET.Element('PAMDataset')
        metadata=ET.SubElement(ET.SubElement(dataset,'PAMRasterBand',band='1'),'Metadata')
        for key,value in fields.items():
            ET.SubElement(metadata,'MDI',key=key).text=value
        Path(str(out)+'.aux.xml').write_bytes(ET.tostring(dataset))
