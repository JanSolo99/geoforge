import numpy as np, sys, json
from osgeo import gdal, osr
import cases
out = sys.argv[1] if len(sys.argv)>1 else 'tif'
for name,(arr,cs) in cases.CASES.items():
    arr=np.asarray(arr,dtype=np.uint8)
    r,c=arr.shape
    ds=gdal.GetDriverByName('GTiff').Create(f'{out}/{name}.tif',c,r,1,gdal.GDT_Byte)
    ds.SetGeoTransform((0,cs,0,r*cs,0,-cs))
    s=osr.SpatialReference(); s.ImportFromEPSG(32631); ds.SetProjection(s.ExportToWkt())
    b=ds.GetRasterBand(1); b.SetNoDataValue(255); b.WriteArray(arr); ds=None
print(len(cases.CASES))
