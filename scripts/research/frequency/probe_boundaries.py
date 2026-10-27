import arcpy,json,os
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
src=P('/tmp/unigis_freq_input_composite_matrix_u8.tif')
for n in [1,2,3,4,15,16,255,256]:
    out='/tmp/unigis_freq_boundary_'+str(n)+'.tif'
    try:
        r=arcpy.gp.EqualToFrequency_sa(src,';'.join([src]*n),P(out))
        ras=arcpy.Raster(P(out))
        print(json.dumps({'count':n,'status':'succeeded','type':ras.pixelType,'nodata':ras.noDataValue,'bands':ras.bandCount,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:print(json.dumps({'count':n,'status':'failed','error':str(e)}))
