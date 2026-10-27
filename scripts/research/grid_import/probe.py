import arcpy,json
arcpy.env.overwriteOutput=True
cases={'ascii_small_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/small.asc', ['INTEGER']], 'ascii_small_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/small.asc', ['FLOAT']], 'ascii_signed_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/signed.asc', ['INTEGER']], 'ascii_signed_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/signed.asc', ['FLOAT']], 'ascii_float_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/float.asc', ['INTEGER']], 'ascii_float_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/float.asc', ['FLOAT']], 'ascii_wide_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/wide.asc', ['INTEGER']], 'ascii_wide_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/wide.asc', ['FLOAT']], 'ascii_center_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/center.asc', ['INTEGER']], 'ascii_center_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/center.asc', ['FLOAT']], 'ascii_no_nd_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/no_nd.asc', ['INTEGER']], 'ascii_no_nd_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/no_nd.asc', ['FLOAT']], 'ascii_short_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/short.asc', ['INTEGER']], 'ascii_short_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/short.asc', ['FLOAT']], 'ascii_extra_INTEGER': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/extra.asc', ['INTEGER']], 'ascii_extra_FLOAT': ['conversion.ASCIIToRaster', '/tmp/unigis_import_probe/extra.asc', ['FLOAT']], 'ascii_existing_dem': ['conversion.ASCIIToRaster', '/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/r2a_dem.asc', ['FLOAT']], 'float_existing_dem': ['conversion.FloatToRaster', '/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/r2f_dem.flt', []], 'float_little': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/little.flt', []], 'float_big': ['conversion.FloatToRaster', '/tmp/unigis_import_probe/big.flt', []]}
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
