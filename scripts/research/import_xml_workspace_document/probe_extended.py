import arcpy, json, datetime, os
root=P('/mnt/media/ArcGIS_Linux/scripts/research/import_xml_workspace_document')
arcpy.env.overwriteOutput=True
src=root+'/extended.gdb'
if arcpy.Exists(src): arcpy.management.Delete(src)
arcpy.management.CreateFileGDB(root,'extended.gdb')
sr=arcpy.SpatialReference(32632)
for name,typ,coords in [('lines','POLYLINE',[[[0,0],[4,3]],[[5,6],[6,7]]]),('areas','POLYGON',[[[0,0],[0,4],[4,4],[4,0],[0,0]]]),('many','MULTIPOINT',[[1,2],[3,4]])]:
    arcpy.management.CreateFeatureclass(src,name,typ,spatial_reference=sr)
    cls={'POLYLINE':arcpy.Polyline,'POLYGON':arcpy.Polygon,'MULTIPOINT':arcpy.Multipoint}[typ]
    a=arcpy.Array([arcpy.Point(*p) for p in coords]) if typ=='MULTIPOINT' else arcpy.Array([arcpy.Array([arcpy.Point(*p) for p in part]) for part in coords])
    with arcpy.da.InsertCursor(src+'/'+name,['SHAPE@']) as c: c.insertRow((cls(a,sr),))
arcpy.management.CreateTable(src,'types')
for name,typ in [('small','SHORT'),('large','LONG'),('single','FLOAT'),('double','DOUBLE'),('stamp','DATE'),('guid','GUID'),('text','TEXT')]:
    arcpy.management.AddField(src+'/types',name,typ,field_length=50)
arcpy.management.AssignDefaultToField(src+'/types','large',42)
fields=['small','large','single','double','stamp','guid','text']
with arcpy.da.InsertCursor(src+'/types',fields,explicit=True) as c:
    c.insertRow((-5,100,1.5,-2.125,datetime.datetime(2024,3,4,5,6,7),'{12345678-1234-5678-9ABC-DEF012345678}','café 空'))
    c.insertRow((None,None,None,None,None,None,''))
# OID holes distinguish preserving IDs from reassigning them.
with arcpy.da.InsertCursor(src+'/types',fields,explicit=True) as c: c.insertRow((1,2,3,4,None,None,'third'))
with arcpy.da.UpdateCursor(src+'/types',['OBJECTID']) as c:
    for r in c:
        if r[0]==2: c.deleteRow()
for storage in ['BINARY','NORMALIZED']:
    arcpy.management.ExportXMLWorkspaceDocument(src,root+'/extended_'+storage.lower()+'.xml','DATA',storage,'NO_METADATA')
arcpy.management.ExportXMLWorkspaceDocument(src,root+'/schema.xml','SCHEMA_ONLY','BINARY','NO_METADATA')
for name,file,mode in [('extended_data','extended_binary.xml','DATA'),('extended_normalized','extended_normalized.xml','DATA'),('schema_default','schema.xml','DATA')]:
    dst=root+'/'+name+'.gdb'
    if arcpy.Exists(dst): arcpy.management.Delete(dst)
    arcpy.management.CreateFileGDB(root,name+'.gdb')
    try:
        r=arcpy.management.ImportXMLWorkspaceDocument(dst,root+'/'+file,mode)
        out={}
        for t in ['types','lines','areas','many']:
            fs=['*'] if t=='types' else ['OBJECTID','SHAPE@JSON']+[f.name for f in arcpy.ListFields(dst+'/'+t) if f.name.lower().startswith('shape_')]
            out[t]={'fields':[(f.name,f.type,f.aliasName,f.length,f.isNullable,f.defaultValue) for f in arcpy.ListFields(dst+'/'+t)],'rows':list(arcpy.da.SearchCursor(dst+'/'+t,fs))}
        print(json.dumps({'case':name,'messages':r.getMessages(),'data':out},default=str,ensure_ascii=False))
    except Exception as e: print(json.dumps({'case':name,'error':str(e)}))
