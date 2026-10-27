import arcpy,json
arcpy.env.overwriteOutput=True
cases={'oor_u8_u8': ['u8', 'oor_u8'], 'oor_u8_f64': ['f64', 'oor_u8'], 'oor_i8_i8': ['i8', 'oor_i8'], 'oor_i8_f64': ['f64', 'oor_i8'], 'oor_u16_u16': ['u16', 'oor_u16'], 'oor_u16_f64': ['f64', 'oor_u16'], 'oor300_u8_u8': ['u8', 'oor300_u8'], 'oor300_u8_f64': ['f64', 'oor300_u8'], 'oorneg_u8_u8': ['u8', 'oorneg_u8'], 'oorneg_u8_f64': ['f64', 'oorneg_u8']}
for name,inputs in cases.items():
    out=P('/tmp/unigis_cbands_nd_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in inputs),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'case':name,'inputs':inputs,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
