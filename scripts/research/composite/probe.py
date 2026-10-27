import arcpy,json
arcpy.env.overwriteOutput=True
base='/mnt/media/ArcGIS_Linux/testdata/raster/'
cases={
 'dem':['dem.tif'],
 'dem_twice':['dem.tif','dem.tif'],
 'float_nodata':['dem.tif','dem_nodata.tif'],
 'nodata_float':['dem_nodata.tif','dem.tif'],
 'int_float':['dem_int16_packbits.tif','dem.tif'],
 'float_int':['dem.tif','dem_int16_packbits.tif'],
 'int_mix':['landuse_u8.tif','patches_i16.tif'],
 'rgb':['rgb_pixel_u8.tif'],
 'rgb_twice':['rgb_pixel_u8.tif','rgb_pixel_u8.tif'],
 'rgb_u16':['rgb_pixel_u8.tif','rgb_planar_u16.tif'],
 'shifted':['dem.tif','zones_shifted.tif'],
 'different_cell':['dem.tif','landuse_u8.tif'],
}
for name,files in cases.items():
    out=P('/tmp/unigis_cbands_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P(base+f) for f in files),out)
        print(json.dumps({'case':name,'inputs':files,'status':'succeeded','messages':r.getMessages()}))
    except Exception as e:
        print(json.dumps({'case':name,'inputs':files,'status':'failed','error':str(e)}))
