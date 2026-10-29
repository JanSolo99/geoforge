"""Read public catalog XML/flags, including qualified item names."""
import json,os
results={}
for key,path in [('before',P('/mnt/media/ArcGIS_Linux/testdata/geoproc/cogo_base.gdb')),('after',P('/tmp/unigis-disable-cogo-probe/source.gdb'))]:
 with arcpy.da.SearchCursor(os.path.join(path,'GDB_Items'),['Name','Definition','ItemInfo','Properties']) as c:results[key]=list(c)
 results[key+'_describe']={n:dict(cogo=getattr(arcpy.Describe(os.path.join(path,n)),'isCOGOEnabled',None),fields=[(f.name,f.required) for f in arcpy.ListFields(os.path.join(path,n))]) for n in ['enabled','empty_enabled','plain','custom']}
with open(P('/tmp/unigis-disable-cogo-catalog.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-disable-cogo-catalog.json')
