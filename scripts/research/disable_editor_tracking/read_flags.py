"""Read saved field flag bytes; file-format check only, never a GP oracle."""
import json,re,struct,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
def flags(gdb):
 text=subprocess.check_output(['ogrinfo','-ro','-q','-sql','SELECT Name FROM GDB_SystemCatalog',str(gdb)],text=True)
 out={}
 for oid,name in re.findall(r'OGRFeature\(GDB_SystemCatalog\):(\d+)\n  Name \(String\) = (.+)',text):
  if name not in ['all_off','creator_off','table_all','local_time','tracked','custom']:continue
  data=(gdb/f'a{int(oid):08x}.gdbtable').read_bytes();start=struct.unpack_from('<Q',data,32)[0];size=struct.unpack_from('<I',data,start)[0];section=data[start+4:start+4+size]
  names=['maker','made','changer','changed'] if name in ['tracked','custom'] else ['creator','created','editor','edited']
  out[name]={}
  for field in names:
   key=bytes([len(field)])+field.encode('utf-16le');at=section.find(key)
   if at<0:continue
   pos=at+len(key);alias=section[pos];pos+=1+2*alias;code=section[pos];pos+=1
   pos+=4 if code==4 else 1
   out[name][field]=dict(offset=start+4+pos,flags=section[pos])
 return out
results={}
for name,before,after in [
 ('base',ROOT/'testdata/geoproc/editor_tracking_base.gdb',Path('/tmp/unigis-disable-editor-tracking-probe/source.gdb')),
 ('extra',ROOT/'testdata/geoproc/editor_tracking_extra.gdb',Path('/tmp/unigis-disable-editor-tracking-extra/extra.gdb'))]:
 a=flags(before);b=flags(after);results[name]=dict(before=a,after=b)
 for table,fields in a.items():print(name,table,{f:(v['flags'],b[table][f]['flags']) for f,v in fields.items()})
Path(__file__).with_name('field_flag_results.json').write_text(json.dumps(results,indent=2)+'\n')
