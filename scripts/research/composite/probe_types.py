import arcpy,json
arcpy.env.overwriteOutput=True
base='/mnt/media/ArcGIS_Linux/testdata/raster/'
cases={
 'u16_i16':['rgb_planar_u16.tif','dem_int16_packbits.tif'],
 'i16_u16':['dem_int16_packbits.tif','rgb_planar_u16.tif'],
 'f64_f32':['dem_f64.tif','dem.tif'],
 'f32_f64':['dem.tif','dem_f64.tif'],
 'i32_u16':['dem_bigtiff_i32.tif','rgb_planar_u16.tif'],
 'i32_float':['dem_bigtiff_i32.tif','dem.tif'],
}
for name,files in cases.items():
    out=P('/tmp/unigis_cbands_'+name+'.tif')
    try:
        r=arcpy.management.CompositeBands(';'.join(P(base+f) for f in files),out)
        print(json.dumps({'case':name,'inputs':files,'status':'succeeded','messages':r.getMessages()}))
    except Exception as e:
        print(json.dumps({'case':name,'inputs':files,'status':'failed','error':str(e)}))
