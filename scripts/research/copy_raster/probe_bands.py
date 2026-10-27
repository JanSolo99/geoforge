import arcpy,json,shutil,os
arcpy.env.overwriteOutput=True
root='/mnt/media/ArcGIS_Linux/testdata/raster/'
for name in ['rgb_pixel_u8.tif','rgb_planar_u16.tif']:
    src='/tmp/unigis_cpr_source_'+name
    shutil.copyfile(P(root+name),P(src))
    for pixel in ['', '32_BIT_FLOAT', '16_BIT_UNSIGNED']:
        out='/tmp/unigis_cpr_'+name[:-4]+'_'+(pixel or 'default')+'.tif'
        try:
            r=arcpy.management.CopyRaster(P(src),P(out),pixel_type=pixel)
            ras=arcpy.Raster(P(out))
            values=arcpy.RasterToNumPyArray(ras)
            print(json.dumps({'source':name,'pixel_type':pixel,'status':'succeeded','bands':ras.bandCount,'type':ras.pixelType,'nodata':ras.noDataValue,'shape':list(values.shape),'first_values':values.reshape(values.shape[0],-1)[:,:8].tolist()}))
            del ras
        except Exception as e:
            print(json.dumps({'source':name,'pixel_type':pixel,'status':'failed','error':str(e)}))
