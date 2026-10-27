import arcpy,json
arcpy.env.overwriteOutput=True
cases={'fallback_f64_u8': ['nd_f64', 'u8'], 'fallback_f32_u8': ['nd_f32', 'u8'], 'fallback_i8_u16': ['nd_i8', 'u16'], 'fallback_u8_i8': ['nd_u8', 'i8'], 'fallback_u32_u8': ['nd_u32', 'u8'], 'fallback_i32_i8': ['nd_i32', 'i8'], 'fallback_i16_u8': ['nd_i16', 'u8'], 'fallback_u16_u8': ['nd_u16', 'u8'], 'fallback_i8_i8': ['nd_i8', 'i8'], 'fallback_u8_u8': ['nd_u8', 'u8'], 'last_f64_u32': ['f64', 'nd_u32'], 'both_u16_i16': ['nd_u16', 'nd_i16'], 'both_i8': ['nd_i8', 'nd_i8'], 'both_u32': ['nd_u32', 'nd_u32'], 'both_f64': ['nd_f64', 'nd_f64'], 'last_i32_u32': ['i32', 'nd_u32']}
for name,inputs in cases.items():
    out=P('/tmp/unigis_cbands_nd_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in inputs),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'case':name,'inputs':inputs,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
