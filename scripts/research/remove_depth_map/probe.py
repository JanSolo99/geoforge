# Public ArcGIS black-box control; every potentially removed file is in scratch/.
import arcpy, hashlib, json, os, shutil
root=P('/mnt/media/ArcGIS_Linux/scripts/research/remove_depth_map')
scratch=root+'/scratch'
if os.path.exists(scratch): shutil.rmtree(scratch)
os.makedirs(scratch)
images=scratch+'/images'
os.makedirs(images)
for name in ['dem','dem_nodata']:
    shutil.copyfile(P('/mnt/media/ArcGIS_Linux/testdata/raster/'+name+'.tif'),images+'/'+name+'.tif')
arcpy.env.overwriteOutput=True
arcpy.management.CreateFileGDB(scratch,'probe.gdb')
gdb=scratch+'/probe.gdb'
md=gdb+'/md'
arcpy.management.CreateMosaicDataset(gdb,'md',arcpy.SpatialReference(32633))
arcpy.management.AddRastersToMosaicDataset(md,'Raster Dataset',images,'UPDATE_CELL_SIZES','UPDATE_BOUNDARY','NO_OVERVIEWS')
rows=list(arcpy.da.SearchCursor(md,['OBJECTID','Name']))
print(json.dumps(dict(install={k:v for k,v in arcpy.GetInstallInfo().items() if k in ['Version','BuildNumber','LicenseLevel']},items=rows)))
def hashes(folder):
    return {name:hashlib.sha256(open(folder+'/'+name,'rb').read()).hexdigest() for name in sorted(os.listdir(folder)) if os.path.isfile(folder+'/'+name) and not name.endswith('.lock')}
# Deliberately invalid .dm payloads: probe path selection/deletion, not the depth format.
for label,where in [('none','OBJECTID < 0'),('first','OBJECTID = '+str(rows[0][0])),('name',"Name = 'dem_nodata'"),('all',''),('invalid','NO_SUCH_FIELD = 1')]:
    for name in ['dem','dem_nodata','unrelated']:
        with open(images+'/'+name+'.dm','wb') as f: f.write(b'UniGIS isolated depth-sidecar sentinel')
    before=hashes(images)
    db_before=hashes(gdb)
    try:
        r=arcpy.management.RemoveDepthMap(md,where)
        status='succeeded'
        messages=r.getMessages()
    except Exception as e:
        status='failed'
        messages=str(e)
    arcpy.management.ClearWorkspaceCache()
    after=hashes(images)
    db_after=hashes(gdb)
    print(json.dumps(dict(case=label,where=where,status=status,messages=messages,removed=sorted(set(before)-set(after)),changed=sorted(k for k in set(before)&set(after) if before[k]!=after[k]),remaining_dm=sorted(n for n in after if n.endswith('.dm')),changed_gdb_files=sorted(k for k in set(db_before)|set(db_after) if db_before.get(k)!=db_after.get(k)))))
# Validation behavior on an empty but valid mosaic and a non-mosaic input.
arcpy.management.CreateMosaicDataset(gdb,'empty',arcpy.SpatialReference(32633))
for path in [gdb+'/empty',images+'/dem.tif']:
    try:
        r=arcpy.management.RemoveDepthMap(path)
        print(json.dumps(dict(input=path,status='succeeded',messages=r.getMessages())))
    except Exception as e: print(json.dumps(dict(input=path,status='failed',messages=str(e))))
