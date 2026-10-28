import json,os
arcpy.CheckOutExtension('Spatial')
arcpy.env.overwriteOutput=True
arcpy.env.workspace=P('/tmp')
fc=arcpy.management.CreateFeatureclass('memory','line_ties','POLYLINE',spatial_reference=arcpy.SpatialReference(32633))[0]
arcpy.management.AddField(fc,'IVAL','LONG')
arcpy.management.AddField(fc,'DVAL','DOUBLE')
with arcpy.da.InsertCursor(fc,['SHAPE@','IVAL','DVAL']) as c:
 for i in [1,2]:
  c.insertRow((arcpy.Polyline(arcpy.Array([arcpy.Point(0,0),arcpy.Point(20,20)]),arcpy.SpatialReference(32633)),i,i+0.25))
arcpy.management.CopyFeatures(fc,P("/mnt/media/ArcGIS_Linux/testdata/geoproc/line_stats_ties.shp"))
