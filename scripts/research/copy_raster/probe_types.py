import arcpy,json,shutil,os
arcpy.env.overwriteOutput=True
root='/mnt/media/ArcGIS_Linux/testdata/raster/composite_matrix/'
for name in ['u8','i8','u16','i16','u32','i32','f32','f64']:
    src='/tmp/unigis_cpr_matrix_source_'+name+'.tif'
    shutil.copyfile(P(root+name+'.tif'),P(src))
    for pixel in ['', '64_BIT']:
        out='/tmp/unigis_cpr_matrix_'+name+'_'+(pixel or 'default')+'.tif'
        try:
            r=arcpy.management.CopyRaster(P(src),P(out),pixel_type=pixel)
            ras=arcpy.Raster(P(out))
            print(json.dumps({'source':name,'pixel_type':pixel,'status':'succeeded','bands':ras.bandCount,'type':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
            del ras
        except Exception as e:
            print(json.dumps({'source':name,'pixel_type':pixel,'status':'failed','error':str(e)}))
