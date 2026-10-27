import arcpy,json
arcpy.env.overwriteOutput=True
cases={'fallback_first_none_u8': ['u8', 'nd_f64', 'i8'], 'fallback_first_none_f64': ['f64', 'nd_u8', 'i8'], 'fallback_first_u8_last_f64': ['nd_u8', 'nd_f64', 'i8'], 'fallback_first_f64_last_u8': ['nd_f64', 'nd_u8', 'i8']}
for name,inputs in cases.items():
    out=P('/tmp/unigis_cbands_nd_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in inputs),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'case':name,'inputs':inputs,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
