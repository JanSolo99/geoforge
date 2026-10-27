import arcpy,json
arcpy.env.overwriteOutput=True
cases={'fallback_combo_i8_u16': ['nd_i8', 'nd_u16', 'u8'], 'fallback_combo_i16_u16': ['nd_i16', 'nd_u16', 'u8'], 'fallback_combo_i8_u8': ['nd_i8', 'nd_u8', 'i8'], 'fallback_combo_i32_f32': ['nd_i32', 'nd_f32', 'u8'], 'fallback_combo_u32_i8': ['nd_u32', 'nd_i8', 'u8']}
for name,inputs in cases.items():
    out=P('/tmp/unigis_cbands_nd_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in inputs),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'case':name,'inputs':inputs,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:
        print(json.dumps({'case':name,'status':'failed','error':str(e)}))
