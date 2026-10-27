import arcpy,json,shutil,os
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
root='/mnt/media/ArcGIS_Linux/testdata/raster/'
cases=[('basic','composite_matrix/u8.tif',['composite_matrix/u8.tif','composite_matrix/i8.tif','composite_matrix/f32.tif'],'SINGLE_BAND'),('value_nd','composite_matrix/nd_u8.tif',['composite_matrix/u8.tif','composite_matrix/i8.tif'],'SINGLE_BAND'),('list_nd','composite_matrix/u8.tif',['composite_matrix/nd_u8.tif','composite_matrix/i8.tif'],'SINGLE_BAND'),('multi_single','copy_raster/first_nodata.tif',['copy_raster/later_nodata.tif'],'SINGLE_BAND'),('multi_multi','copy_raster/first_nodata.tif',['copy_raster/later_nodata.tif'],'MULTI_BAND')]
def private(name):
    dst='/tmp/unigis_freq_input_'+name.replace('/','_')
    if not os.path.exists(P(dst)):shutil.copyfile(P(root+name),P(dst))
    return P(dst)
for tool in ['EqualToFrequency','GreaterThanFrequency','LessThanFrequency']:
    for name,value,rasters,multi in cases:
        out='/tmp/unigis_freq_'+tool+'_'+name+'.tif'
        try:
            r=getattr(arcpy.gp,tool+'_sa')(private(value),';'.join(private(x) for x in rasters),P(out),multi)
            ras=arcpy.Raster(P(out))
            print(json.dumps({'tool':tool,'case':name,'status':'succeeded','bands':ras.bandCount,'pixel_type':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist(),'extent':[ras.extent.XMin,ras.extent.YMin,ras.extent.XMax,ras.extent.YMax],'cellsize':[ras.meanCellWidth,ras.meanCellHeight]}))
            del ras
        except Exception as e:
            print(json.dumps({'tool':tool,'case':name,'status':'failed','error':str(e)}))
