"""Create the small, public ArcGIS source fixture, without applying the GPS tool."""
import os
root=P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
gdb=arcpy.management.CreateFileGDB(root,'gps_metadata_base.gdb').getOutput(0)
sr=arcpy.SpatialReference(32633)
for name,kind in [('point','POINT'),('point_zm','POINT'),('line','POLYLINE'),('polygon','POLYGON'),('multipoint','MULTIPOINT'),('existing_receiver','POINT')]:
 fc=arcpy.management.CreateFeatureclass(gdb,name,kind,has_z='ENABLED' if name=='point_zm' else 'DISABLED',has_m='ENABLED' if name=='point_zm' else 'DISABLED',spatial_reference=sr).getOutput(0)
 arcpy.management.AddField(fc,'KEY','LONG',field_alias='Source key')
 pts=[arcpy.Point(500000,5000000,12,3),arcpy.Point(500000,5000010,14,4),arcpy.Point(500010,5000010,16,5),arcpy.Point(500010,5000000,18,6),arcpy.Point(500000,5000000,12,3)]
 geom=arcpy.PointGeometry(pts[0],sr,name=='point_zm',name=='point_zm') if kind=='POINT' else getattr(arcpy,{'POLYLINE':'Polyline','POLYGON':'Polygon','MULTIPOINT':'Multipoint'}[kind])(arcpy.Array(pts),sr)
 with arcpy.da.InsertCursor(fc,['KEY','SHAPE@']) as c:
  c.insertRow((7,geom));c.insertRow((11,None))
 if name=='existing_receiver':
  arcpy.management.AddField(fc,'ESRIGNSS_RECEIVER','TEXT',field_length=7,field_alias='Custom alias')
  with arcpy.da.UpdateCursor(fc,['ESRIGNSS_RECEIVER']) as c:
   for row in c:row[0]='Keep';c.updateRow(row)
arcpy.management.CreateTable(gdb,'table_only')
print('Created gps_metadata_base.gdb')
