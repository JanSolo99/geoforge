import arcpy,json,os,shutil
arcpy.env.overwriteOutput=True
src='/tmp/unigis_cpr_first_nodata.tif'
shutil.copyfile(P('/mnt/media/ArcGIS_Linux/testdata/raster/copy_raster/first_nodata.tif'),P(src))
out='/tmp/unigis_cpr_first_nodata_result.tif'
r=arcpy.management.CopyRaster(P(src),P(out))
ras=arcpy.Raster(P(out))
print(json.dumps({'status':'succeeded','bands':ras.bandCount,'type':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist(),'aux_exists':os.path.exists(P(out+'.aux.xml'))}))
