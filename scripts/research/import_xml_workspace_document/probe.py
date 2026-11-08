import arcpy, json, os
root=P('/mnt/media/ArcGIS_Linux/scripts/research/import_xml_workspace_document')
arcpy.env.overwriteOutput=True
src=root+'/source.gdb'
if arcpy.Exists(src): arcpy.management.Delete(src)
arcpy.management.CreateFileGDB(root,'source.gdb')
arcpy.management.CreateTable(src,'labels')
arcpy.management.AddField(src+'/labels','number','LONG',field_alias='Count')
arcpy.management.AddField(src+'/labels','label','TEXT',field_length=40)
with arcpy.da.InsertCursor(src+'/labels',['number','label']) as c:
    c.insertRow((4,'A & B <town>'))
    c.insertRow((None,None))
arcpy.management.CreateFeatureclass(src,'points','POINT',spatial_reference=arcpy.SpatialReference(32632))
arcpy.management.AddField(src+'/points','number','SHORT')
with arcpy.da.InsertCursor(src+'/points',['SHAPE@XY','number']) as c:
    c.insertRow(((1,2),3))
for storage in ['BINARY','NORMALIZED']:
    arcpy.management.ExportXMLWorkspaceDocument(src,root+'/'+storage.lower()+'.xml','DATA',storage,'NO_METADATA')
for mode in ['DATA','SCHEMA_ONLY']:
    dst=root+'/'+mode.lower()+'.gdb'
    if arcpy.Exists(dst): arcpy.management.Delete(dst)
    arcpy.management.CreateFileGDB(root,mode.lower()+'.gdb')
    result=arcpy.management.ImportXMLWorkspaceDocument(dst,root+'/binary.xml',mode)
    print(json.dumps({'mode':mode,'messages':result.getMessages(),'tables':{n:list(arcpy.da.SearchCursor(dst+'/'+n,['*'])) for n in ['labels','points']}}))
# Repeat without overwrite, so the native importer can implement default collision policy.
arcpy.env.overwriteOutput=False
r=arcpy.management.ImportXMLWorkspaceDocument(root+'/data.gdb',root+'/binary.xml')
print(json.dumps({'collision':r.getMessages(),'names':arcpy.da.Walk(root+'/data.gdb').__next__()[2]}))
