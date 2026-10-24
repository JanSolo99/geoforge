import numpy as np
from osgeo import gdal
gdal.UseExceptions()
D="testdata/arcgis-reference/"
def load(p):
    ds=gdal.Open(p); b=ds.GetRasterBand(1); return b.ReadAsArray().astype(np.float64), b.GetNoDataValue()
a,_=load("testdata/raster/dem.tif"); want,wnd=load(D+"slice_ei5.tif")
lo,hi=a.min(),a.max(); n=5
z=np.minimum(np.floor((a-lo)/((hi-lo)/n)),n-1)+1
bad=np.argwhere(z!=want)
print("min",repr(lo),"max",repr(hi))
for r,c in bad: print(a[r,c], "want",want[r,c],"ours",z[r,c], "k-pos",(a[r,c]-lo)/((hi-lo)/n))
k=(a-lo)/((hi-lo)/n)
frac=k-np.floor(k)
near=(frac>0.99)&(k<n)
print("flipped max dist below boundary:", (1-frac[(z!=want)]).max())
print("unflipped below boundary, closest:", sorted((1-frac[near&(z==want)]))[:5])
# alternative: float32 bins
w=(hi-lo)
for bins in (1024,2048,4096,8192,10000,16384,65536,100000):
    zz=np.minimum(np.floor(np.floor((a-lo)/w*bins)/(bins/n)),n-1)+1
    print(bins,int((zz!=want).sum()))
