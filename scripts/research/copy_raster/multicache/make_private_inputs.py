from pathlib import Path
import json,shutil,xml.etree.ElementTree as ET
folder=Path('/tmp/unigis-copy-multicache-controls');folder.mkdir(exist_ok=True)
source=Path('/mnt/media/ArcGIS_Linux/testdata/raster/copy_statistics/multi_uncached.tif')
a={'MINIMUM':'1','MAXIMUM':'4','MEAN':'2.5','STDDEV':'1.2','COUNT':'3.000000'}
b={'MINIMUM':'10','MAXIMUM':'40','MEAN':'25','STDDEV':'12','COUNT':'4.000000'}
variants={'first_full':[a,{}],'second_full':[{},b],'split_extrema':[{'MINIMUM':'1'},{'MAXIMUM':'40'}],
 'first_full_second_extrema':[a,{k:v for k,v in b.items() if k in ['MINIMUM','MAXIMUM']}],
 'first_extrema_second_full':[{k:v for k,v in a.items() if k in ['MINIMUM','MAXIMUM']},b],
 'second_missing_extrema':[a,{k:v for k,v in b.items() if k in ['MEAN','STDDEV']}],
 'domain_only':[a,{}],'single_quotes':[a,b]}
for name,bands in variants.items():
 xml=ET.Element('PAMDataset')
 for index,fields in enumerate(bands,1):
  band=ET.SubElement(xml,'PAMRasterBand',band=str(index));meta=ET.SubElement(band,'Metadata',**({'domain':'unigis-test'} if name=='domain_only' else {}))
  for key,value in fields.items():ET.SubElement(meta,'MDI',key='STATISTICS_'+key).text=value
 text=ET.tostring(xml,encoding='unicode')
 if name=='single_quotes':text=text.replace('"',"'")
 for suffix in ['', '_native']:
  out=folder/(name+suffix+'.tif');shutil.copyfile(source,out);Path(str(out)+'.aux.xml').write_text(text)
(folder/'inputs.json').write_text(json.dumps(variants,indent=2)+'\n')
