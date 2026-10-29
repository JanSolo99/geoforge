"""One-time public ArcGIS fixture creation; refuses to replace existing fixtures."""
import os
root=P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
for name in ['fctg_empty.gdb','fctg_seed.gdb','fctg_names']:
 if os.path.exists(root+'/'+name):raise RuntimeError('Fixture already exists: '+name)
arcpy.management.CreateFileGDB(root,'fctg_empty.gdb')
arcpy.management.Copy(P('/tmp/unigis-fctg-probe/single.gdb'),root+'/fctg_seed.gdb')
arcpy.management.CreateFeatureDataset(root+'/fctg_seed.gdb','group',arcpy.Describe(root+'/parcels.shp').spatialReference)
os.makedirs(root+'/fctg_names')
arcpy.management.CopyFeatures(root+'/wells.shp',root+'/fctg_names/9 bad-name.shp')
print('Created import seed/name fixtures once')
