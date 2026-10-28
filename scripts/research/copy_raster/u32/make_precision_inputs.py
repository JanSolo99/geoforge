from pathlib import Path
from osgeo import gdal
gdal.UseExceptions()
profiles=[('u32_neg_float_edge','u32',4278190079),('u32_neg_near_min','u32',2147483775),('u32_positive_edge','u32',16777217),('i32_neg_float_edge','i32',-16777217),('i32_neg_near_min','i32',-2147483647),('f64_neg_float_edge','f64',-16777217)]
for name,typ,marker in profiles:
 src=gdal.Open(f'testdata/raster/composite_matrix/{typ}.tif')
 out=gdal.GetDriverByName('GTiff').Create(f'/tmp/unigis-u32-export-controls/{name}.tif',3,2,1,src.GetRasterBand(1).DataType)
 out.SetGeoTransform(src.GetGeoTransform());out.SetProjection(src.GetProjection())
 b=out.GetRasterBand(1);b.WriteArray(src.ReadAsArray());b.SetNoDataValue(marker);b=None;out=None
