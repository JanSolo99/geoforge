import arcpy,json,os,shutil
arcpy.env.overwriteOutput=True
source='/mnt/media/ArcGIS_Linux/testdata/arcgis-reference/ascimp_small_integer.tif'
for code in [4326,32633,3857,7844,7856]:
    linux='/tmp/unigis_define_'+str(code)+'.tif'
    shutil.copyfile(P(source),P(linux))
    if os.path.exists(P(source+'.aux.xml')): shutil.copyfile(P(source+'.aux.xml'),P(linux+'.aux.xml'))
    try:
        r=arcpy.management.DefineProjection(P(linux),arcpy.SpatialReference(code))
        ras=arcpy.Raster(P(linux))
        print(json.dumps({'code':code,'status':'succeeded','epsg':ras.spatialReference.factoryCode,'pixelType':ras.pixelType,'nodata':ras.noDataValue,'values':arcpy.RasterToNumPyArray(ras).tolist(),'messages':r.getMessages()}))
        del ras
    except Exception as e:
        print(json.dumps({'code':code,'status':'failed','error':str(e)}))
