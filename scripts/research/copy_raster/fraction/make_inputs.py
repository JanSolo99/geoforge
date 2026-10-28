from osgeo import gdal
gdal.UseExceptions()
for marker in [-1.5,-1.75,1.5,1.75,0.5]:
 src=gdal.Open('testdata/raster/composite_matrix/f64.tif');a=src.ReadAsArray();a[0,1]=marker
 out=gdal.GetDriverByName('GTiff').Create(f'/tmp/unigis-copy-fraction-controls/source_{marker}.tif',3,2,1,gdal.GDT_Float64)
 out.SetGeoTransform(src.GetGeoTransform());out.SetProjection(src.GetProjection())
 b=out.GetRasterBand(1);b.WriteArray(a);b.SetNoDataValue(marker);b=None;out=None
