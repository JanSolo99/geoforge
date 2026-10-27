import arcpy,json
arcpy.env.overwriteOutput=True
for names in [('u8','shifted'),('shifted','u8'),('f32','shifted'),('i16','shifted')]:
    out=P('/tmp/unigis_composite_matrix/pad_'+names[0]+'_'+names[1]+'.tif')
    r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in names),out)
    ras=arcpy.Raster(out)
    print(json.dumps({'inputs':names,'pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist()}))
    del ras
