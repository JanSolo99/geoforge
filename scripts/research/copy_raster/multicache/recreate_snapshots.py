"""Recreate controlled file-format inputs, retaining deliberate partial/stale caches.

Optional argument selects a private output directory. Never recreate repository
fixtures while tests/oracle generation are using them. GDAL only converts TIFF
sample storage to F64; ArcGIS alone supplies geoprocessing expectations.
"""
from pathlib import Path
import json
import shutil
import sys
import xml.etree.ElementTree as ET
from osgeo import gdal
gdal.UseExceptions()
root=Path(__file__).resolve().parents[4]
folder=Path(sys.argv[1]) if len(sys.argv)>1 else root/'testdata/raster/copy_multicache'
folder.mkdir(parents=True,exist_ok=True)
variants=json.loads(Path(__file__).with_name('inputs.json').read_text())
source=root/'testdata/raster/copy_statistics/multi_uncached.tif'
for name,bands in variants.items():
    xml=ET.Element('PAMDataset')
    for index,fields in enumerate(bands,1):
        band=ET.SubElement(xml,'PAMRasterBand',band=str(index))
        metadata=ET.SubElement(band,'Metadata',**({'domain':'unigis-test'} if name=='domain_only' else {}))
        for key,value in fields.items():
            ET.SubElement(metadata,'MDI',key='STATISTICS_'+key).text=value
    text=ET.tostring(xml,encoding='unicode')
    if name=='single_quotes':text=text.replace('"',"'")
    output=folder/(name+'.tif')
    shutil.copyfile(source,output)
    Path(str(output)+'.aux.xml').write_text(text)
for name in ['first_full','second_full','single_quotes']:
    source=folder/(name+'.tif');output=folder/(name+'_f64.tif')
    converted=gdal.Translate(str(output),str(source),outputType=gdal.GDT_Float64)
    converted=None
    shutil.copyfile(Path(str(source)+'.aux.xml'),Path(str(output)+'.aux.xml'))
