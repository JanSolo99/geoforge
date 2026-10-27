import arcpy,json
arcpy.env.overwriteOutput=True
cases={'ascii_all_nd_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/all_nd.asc', ['INTEGER']], 'ascii_all_nd_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/all_nd.asc', ['FLOAT']], 'ascii_default': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/small.asc', []], 'float_all_nd': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/all_nd.flt', []]}
for name,(tool,src,extra) in cases.items():
    out=P('/tmp/unigis_import_probe/'+name+'.tif')
    try:
        fn=getattr(arcpy.conversion,tool.split('.')[-1])
        r=fn(P(src),out,*extra)
        ras=arcpy.Raster(out)
        row={'case':name,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'epsg':ras.spatialReference.factoryCode,'rows':ras.height,'cols':ras.width}
        if ras.height*ras.width<=20: row['values']=arcpy.RasterToNumPyArray(ras).tolist()
        print(json.dumps(row))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
