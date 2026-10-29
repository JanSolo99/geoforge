"""Create a tiny public raster fixture and delete only its disposable copy."""
import numpy,os,shutil,json
source=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/delete_multiple')
r=arcpy.NumPyArrayToRaster(numpy.array([[1,2],[3,4]],dtype=numpy.int16),arcpy.Point(500000,5000000),1,1)
r.save(os.path.join(source,'grid.tif'));del r
arcpy.management.CalculateStatistics(os.path.join(source,'grid.tif'))
folder=P('/tmp/unigis-delete-raster-probe');os.makedirs(folder,exist_ok=False)
for name in os.listdir(source):
 if name.startswith('grid.'):shutil.copy2(os.path.join(source,name),os.path.join(folder,name))
open(os.path.join(folder,'grid.txt'),'w').write('preserve')
before=sorted(os.listdir(folder))
r=arcpy.management.DeleteMultiple([[os.path.join(folder,'grid.tif')]])
result=dict(before=before,after=sorted(os.listdir(folder)),output=r.getOutput(0),messages=arcpy.GetMessages())
with open(P('/tmp/unigis-delete-raster-probe.json'),'w') as f:json.dump(result,f,indent=2)
print(result)
