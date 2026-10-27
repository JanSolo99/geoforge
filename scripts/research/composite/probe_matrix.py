import arcpy,json,itertools
arcpy.env.overwriteOutput=True
names=['u8','i8','u16','i16','u32','i32','f32','f64']
for a,b in itertools.product(names,repeat=2):
    out=P('/tmp/unigis_composite_matrix/out_'+a+'_'+b+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P('/tmp/unigis_composite_matrix/'+n+'.tif') for n in [a,b]),out)
        ras=arcpy.Raster(out)
        print(json.dumps({'a':a,'b':b,'status':'succeeded','pixelType':ras.pixelType,'nodata':ras.noDataValue}))
        del ras
    except Exception as e:
        print(json.dumps({'a':a,'b':b,'status':'failed','error':str(e)}))
