import arcpy, json
root=P('/mnt/media/ArcGIS_Linux/scripts/research/import_xml_workspace_document')
checks=[]
for name in ['labels','points','lines','areas','many','types']:
    src=root+('/source.gdb/' if name in ['labels','points'] else '/extended.gdb/')+name
    dst=root+'/native.gdb/'+name
    def schema(path):
        return [(f.name,f.type,f.aliasName,f.length,f.isNullable,f.defaultValue) for f in arcpy.ListFields(path)]
    fields=[f.name for f in arcpy.ListFields(src) if f.type!='Geometry']
    if name not in ['labels','types']: fields.append('SHAPE@JSON')
    def values(path):
        rows=[]
        for r in arcpy.da.SearchCursor(path,fields):
            r=list(r)
            if fields[-1]=='SHAPE@JSON': r[-1]=json.loads(r[-1]) if r[-1] else None
            rows.append(r)
        return rows
    schema_equal=schema(src)==schema(dst)
    rows_equal=values(src)==values(dst)
    sr_equal=True if name in ['labels','types'] else arcpy.Describe(src).spatialReference.exportToString()==arcpy.Describe(dst).spatialReference.exportToString()
    checks.append(dict(table=name,schema_equal=schema_equal,rows_equal=rows_equal,spatial_reference_equal=sr_equal))
print(json.dumps(dict(arcgis=arcpy.GetInstallInfo(),checks=checks),default=str))
assert all(c['schema_equal'] and c['rows_equal'] and c['spatial_reference_equal'] for c in checks), checks
