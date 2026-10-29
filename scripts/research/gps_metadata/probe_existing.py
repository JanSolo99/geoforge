"""Existing schema/domain and non-geodatabase refusal controls."""
import os,json
root=P('/tmp/unigis-gps-existing');os.makedirs(root,exist_ok=True)
sr=arcpy.SpatialReference(32633)
results={}
for case in ['existing_field','wrong_type','existing_domain','wrong_domain_type','table','shapefile']:
 gdb=arcpy.management.CreateFileGDB(root,case+'.gdb').getOutput(0)
 fc=arcpy.management.CreateTable(gdb,'data').getOutput(0) if case=='table' else arcpy.management.CreateFeatureclass(root if case=='shapefile' else gdb,'data.shp' if case=='shapefile' else 'data','POINT',spatial_reference=sr).getOutput(0)
 if case in ['existing_field','wrong_type']:
  arcpy.management.AddField(fc,'ESRIGNSS_RECEIVER','TEXT' if case=='existing_field' else 'LONG',field_length=7,field_alias='Custom alias')
  with arcpy.da.InsertCursor(fc,['SHAPE@XY','ESRIGNSS_RECEIVER']) as c:c.insertRow(((500000,5000000),'Keep' if case=='existing_field' else 42))
 if case in ['existing_domain','wrong_domain_type']:
  arcpy.management.CreateDomain(gdb,'ESRI_FIX_TYPE_DOMAIN','Custom domain','SHORT' if case=='existing_domain' else 'TEXT','CODED')
  arcpy.management.AddCodedValueToDomain(gdb,'ESRI_FIX_TYPE_DOMAIN',9 if case=='existing_domain' else 'nine','Custom')
 try:arcpy.management.AddGPSMetadataFields(fc);error=None
 except Exception as e:error=str(e)
 results[case]={'error':error,'messages':arcpy.GetMessages(),'fields':[{'name':f.name,'type':f.type,'alias':f.aliasName,'length':f.length,'domain':f.domain} for f in arcpy.ListFields(fc)],'domains':[{'name':d.name,'type':d.type,'description':d.description,'values':d.codedValues if d.domainType=='CodedValue' else list(d.range)} for d in arcpy.da.ListDomains(gdb)]}
 arcpy.management.Delete(gdb)
 if case=='shapefile':arcpy.management.Delete(fc)
with open(P('/tmp/unigis-gps-existing.json'),'w') as f:json.dump(results,f,indent=2)
print('Saved /tmp/unigis-gps-existing.json')
