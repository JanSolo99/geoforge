import arcpy,json
arcpy.env.overwriteOutput=True
cases={'ascii_near_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/near.asc', ['INTEGER']], 'ascii_near_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/near.asc', ['FLOAT']], 'ascii_near_header_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/near_header.asc', ['INTEGER']], 'ascii_near_header_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/near_header.asc', ['FLOAT']], 'float_no_byteorder': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_no_byteorder.flt', []], 'float_no_nd': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_no_nd.flt', []], 'float_center': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_center.flt', []], 'float_near_header': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_near_header.flt', []], 'float_short': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_short.flt', []], 'float_extra': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/float_extra.flt', []]}
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
