import json
fc=P('/tmp/unigis-gps-native-reader-fixed.gdb/point')
result={'fields':[{'name':f.name,'aliasName':f.aliasName,'type':f.type,'length':f.length,'isNullable':f.isNullable,'domain':f.domain} for f in arcpy.ListFields(fc)],'rows':list(arcpy.da.SearchCursor(fc,['OID@','KEY','SHAPE@JSON','ESRIGNSS_RECEIVER','ESRIGNSS_CP2_NUMSATS'])),'domains':[{'name':d.name,'type':d.type,'values':d.codedValues if d.domainType=='CodedValue' else list(d.range)} for d in arcpy.da.ListDomains(P('/tmp/unigis-gps-native-reader-fixed.gdb'))]}
with open(P('/tmp/unigis-gps-native-reader-fixed.json'),'w') as f:json.dump(result,f,indent=2)
print('Read native output with ArcGIS:',len(result['fields']),'fields,',len(result['rows']),'rows,',len(result['domains']),'domains')
