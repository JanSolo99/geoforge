import arcpy,json
arcpy.env.overwriteOutput=True
cases={'padding_u8nd_shifted': ['nd_u8', 'shifted'], 'padding_f64nd_shifted': ['nd_f64', 'shifted'], 'padding_u8_shiftednd': ['u8', 'shifted_nd_u8'], 'padding_middle_nd': ['u8', 'shifted_nd_u8', 'nd_f64']}
for name,inputs in cases.items():
    out=P('/tmp/unigis_cbands_nd_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in inputs),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'case':name,'inputs':inputs,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
