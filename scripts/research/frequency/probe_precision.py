import arcpy,json,shutil,os
arcpy.env.overwriteOutput=True
arcpy.CheckOutExtension('Spatial')
root='/mnt/media/ArcGIS_Linux/testdata/raster/'
cases=[('f64_precision','composite_matrix/f64.tif',['composite_matrix/f32.tif','composite_matrix/f64.tif'],'SINGLE_BAND'),('f32_precision','composite_matrix/f32.tif',['composite_matrix/f32.tif','composite_matrix/f64.tif'],'SINGLE_BAND'),('u32_precision','composite_matrix/u32.tif',['composite_matrix/f32.tif','composite_matrix/u32.tif'],'SINGLE_BAND')]
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
