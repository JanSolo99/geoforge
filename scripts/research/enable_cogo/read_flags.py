"""Saved file-format field flags, not a geoprocessing behavior oracle."""
import struct,re,json,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
def read(gdb):
 catalog=subprocess.check_output(['ogrinfo','-ro','-q','-sql','SELECT Name FROM GDB_SystemCatalog',str(gdb)],text=True)
 out={}
 for oid,name in re.findall(r'OGRFeature\(GDB_SystemCatalog\):(\d+)\n  Name \(String\) = (.+)',catalog):
  if name not in ['plain','partial','existing','enabled']:continue
  data=(gdb/f'a{int(oid):08x}.gdbtable').read_bytes();start=struct.unpack_from('<Q',data,32)[0];size=struct.unpack_from('<I',data,start)[0];section=data[start+4:start+4+size];out[name]={}
  for field in ['Direction','Distance','Radius','ArcLength','Radius2']:
   key=bytes([len(field)])+field.encode('utf-16le');at=section.find(key)
   if at<0:continue
   pos=at+len(key);alias=section[pos];pos+=1+2*alias;code=section[pos];pos+=1
   pos+=4 if code==4 else 1
   out[name][field]=section[pos]
 return out
results=dict(before=read(ROOT/'testdata/geoproc/enable_cogo_base.gdb'),after=read(Path('/tmp/unigis-enable-cogo-probe/source.gdb')))
Path(__file__).with_name('field_flag_results.json').write_text(json.dumps(results,indent=2)+'\n')
print(json.dumps(results))
