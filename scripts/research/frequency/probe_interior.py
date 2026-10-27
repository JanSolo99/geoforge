import arcpy,json,os
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
src=P('/tmp/unigis_freq_input_composite_matrix_u8.tif')
for n in [5,7,14,127,128,254]:
    out='/tmp/unigis_freq_boundary_'+str(n)+'.tif'
    try:
        r=arcpy.gp.EqualToFrequency_sa(src,';'.join([src]*n),P(out))
        ras=arcpy.Raster(P(out))
        print(json.dumps({'count':n,'status':'succeeded','type':ras.pixelType,'nodata':ras.noDataValue,'bands':ras.bandCount,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
        del ras
    except Exception as e:print(json.dumps({'count':n,'status':'failed','error':str(e)}))

src=P('/tmp/unigis_freq_input_composite_matrix_nd_u8.tif')
out=P('/tmp/unigis_freq_boundary_255_nodata.tif')
r=arcpy.gp.EqualToFrequency_sa(src,';'.join([src]*255),out)
ras=arcpy.Raster(out)
print(json.dumps({'case':'nd_255','count':255,'status':'succeeded','type':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
