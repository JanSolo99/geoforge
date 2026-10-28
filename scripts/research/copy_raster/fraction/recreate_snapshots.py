"""Recreate source pixels with GDAL (format fixture creation only).

Retain the committed authentic ArcGIS cache beside cached_u8.tif.
Run from the repository root while no tests/oracle generation use these files.
"""
from pathlib import Path
import numpy as np
from osgeo import gdal
gdal.UseExceptions()
folder=Path('testdata/raster/copy_fraction')
folder.mkdir(parents=True,exist_ok=True)
source=gdal.Open('testdata/raster/composite_matrix/f64.tif')

def write(name,values,marker,transform=None):
    a=np.asarray(values,dtype=np.float64)
    out=gdal.GetDriverByName('GTiff').Create(str(folder/(name+'.tif')),a.shape[1],a.shape[0],1,gdal.GDT_Float64)
    out.SetGeoTransform(transform or source.GetGeoTransform())
    out.SetProjection(source.GetProjection())
    b=out.GetRasterBand(1);b.SetNoDataValue(marker);b.WriteArray(a)
    b=None;out=None

for marker in [-1.5,-1.75,1.5,1.75,0.5]:
    a=source.ReadAsArray();a[0,1]=marker
    write('source_'+str(marker),a,marker)
transform=[500000.,10.,0.,5000010.,0.,-10.]
write('pixel_ties',[[-2.5,-1.5,-.5,.5,1.5,2.5,-9999.]],-9999.,transform)
write('pixel_quarters',[[-2.75,-2.25,-1.75,-1.25,-.75,-.25,.25,.75,-9999.]],-9999.,transform)
write('payload_tie',[[-2.5,-1.5,-.5,.5,1.5,2.5,-2.5]],-2.5,transform)
source=None
cached=gdal.Translate(str(folder/'cached_u8.tif'),'testdata/raster/composite_matrix/nd_u8.tif')
cached=None
