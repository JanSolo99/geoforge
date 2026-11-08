# Run after probe.py. Isolate why successful no-op calls changed GDB_Items.
import arcpy, hashlib, json, os, xml.etree.ElementTree as ET
root=P('/mnt/media/ArcGIS_Linux/scripts/research/remove_depth_map/scratch')
gdb=root+'/probe.gdb'
md=gdb+'/md'
def hashes():
    return {n:hashlib.sha256(open(gdb+'/'+n,'rb').read()).hexdigest() for n in os.listdir(gdb) if os.path.isfile(gdb+'/'+n) and not n.endswith('.lock')}
def processes(xml):
    return [n.attrib for n in ET.fromstring(xml).iter() if n.tag.rsplit('}',1)[-1]=='Process']
for enabled in [True,False]:
    arcpy.SetLogHistory(enabled)
    before_xml=arcpy.metadata.Metadata(md).xml
    before=hashes()
    r=arcpy.management.RemoveDepthMap(md,'OBJECTID < 0')
    after_xml=arcpy.metadata.Metadata(md).xml
    after=hashes()
    before_processes=processes(before_xml)
    after_processes=processes(after_xml)
    print(json.dumps(dict(log_history=enabled,messages=r.getMessages(),metadata_changed=before_xml!=after_xml,process_count_before=len(before_processes),process_count_after=len(after_processes),new_processes=after_processes[len(before_processes):],changed_gdb_files=sorted(n for n in set(before)|set(after) if before.get(n)!=after.get(n)))))
