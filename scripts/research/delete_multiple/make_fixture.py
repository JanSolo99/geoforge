"""Create a public source fixture; all DeleteMultiple references operate on copies."""
import os,shutil
root=P('/mnt/media/ArcGIS_Linux/testdata/geoproc/delete_multiple')
os.makedirs(root,exist_ok=False)
# Reuse only completed disposable probe inputs; no repeat creation/oracle deletions.
source=P('/tmp/unigis-delete-multiple-oracle/missing')
for name in os.listdir(source):
 if not name.endswith('.lock'):shutil.copy2(os.path.join(source,name),os.path.join(root,name))
for stem in ['points spaced','points.other']:
 for ext in ['shp','dbf','shx','prj','cpg']:
  shutil.copy2(os.path.join(root,'points.'+ext),os.path.join(root,stem+'.'+ext))
gdb=arcpy.management.CreateFileGDB(root,'data.gdb').getOutput(0)
arcpy.management.CopyFeatures(os.path.join(root,'points.shp'),os.path.join(gdb,'points'))
arcpy.management.CreateTable(gdb,'table')
print('Created delete_multiple fixture')
