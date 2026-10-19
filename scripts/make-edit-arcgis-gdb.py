# Builds testdata/gdb-reference/edit_arcgis.gdb in ArcGIS Pro (the geodatabase the
# editing tests change): run through the ArcGIS bridge with
#
#   uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/make-edit-arcgis-gdb.py
#
# parcels: polygons (Shape_Length/Shape_Area), NAME indexed (IX_NAME), rows 4 and 9 deleted,
#          row 6 updated (so the table has a .freelist);
# roads:   polyline Z with Shape_Length;
# pts64:   points with 64-bit OBJECTIDs (ArcGIS Pro 3.2 table format), row 3 deleted;
# log:     a table with N indexed (IX_N).
import datetime
import os

REF = P('/mnt/media/ArcGIS_Linux/testdata/gdb-reference')
g = os.path.join(REF, 'edit_arcgis.gdb')
if arcpy.Exists(g):
    arcpy.management.Delete(g)
arcpy.management.CreateFileGDB(REF, 'edit_arcgis.gdb')
sr = arcpy.SpatialReference(28992)

arcpy.management.CreateFeatureclass(g, 'parcels', 'POLYGON', spatial_reference=sr)
p = os.path.join(g, 'parcels')
arcpy.management.AddField(p, 'NAME', 'TEXT', field_length=30)
arcpy.management.AddField(p, 'CLASS', 'SHORT')
arcpy.management.AddField(p, 'VALUE', 'DOUBLE')
arcpy.management.AddField(p, 'SURVEYED', 'DATE')
with arcpy.da.InsertCursor(p, ['SHAPE@', 'NAME', 'CLASS', 'VALUE', 'SURVEYED']) as c:
    for i in range(20):
        x = 150000 + (i % 5) * 100
        y = 450000 + (i // 5) * 100
        ring = [arcpy.Point(x, y), arcpy.Point(x, y + 80), arcpy.Point(x + 80, y + 80), arcpy.Point(x + 80, y)]
        c.insertRow([arcpy.Polygon(arcpy.Array(ring), sr), 'parcel %d' % i, i % 3, i * 1.5,
                     datetime.datetime(2020, 1, 1 + i) if i % 4 else None])
arcpy.management.AddIndex(p, 'NAME', 'IX_NAME')
with arcpy.da.UpdateCursor(p, ['OID@', 'VALUE']) as c:
    for r in c:
        if r[0] in (4, 9):
            c.deleteRow()
        elif r[0] == 6:
            r[1] = -1
            c.updateRow(r)

arcpy.management.CreateFeatureclass(g, 'roads', 'POLYLINE', has_z='ENABLED', spatial_reference=sr)
p = os.path.join(g, 'roads')
arcpy.management.AddField(p, 'RNAME', 'TEXT', field_length=20)
with arcpy.da.InsertCursor(p, ['SHAPE@', 'RNAME']) as c:
    for i in range(6):
        pts = [arcpy.Point(150000 + i * 50, 449900, 1 + i), arcpy.Point(150040 + i * 50, 449990, 2 + i),
               arcpy.Point(150100 + i * 50, 450200, 3)]
        c.insertRow([arcpy.Polyline(arcpy.Array(pts), sr, True), 'road %d' % i])

arcpy.management.CreateFeatureclass(g, 'pts64', 'POINT', spatial_reference=sr, oid_type='64_BIT')
p = os.path.join(g, 'pts64')
arcpy.management.AddField(p, 'LABEL', 'TEXT', field_length=10)
with arcpy.da.InsertCursor(p, ['SHAPE@XY', 'LABEL']) as c:
    for i in range(8):
        c.insertRow([(150000 + i * 30, 450500 + i * 7), 'p%d' % i])
with arcpy.da.UpdateCursor(p, ['OID@']) as c:
    for r in c:
        if r[0] == 3:
            c.deleteRow()

arcpy.management.CreateTable(g, 'log')
p = os.path.join(g, 'log')
arcpy.management.AddField(p, 'MSG', 'TEXT', field_length=40)
arcpy.management.AddField(p, 'N', 'LONG')
with arcpy.da.InsertCursor(p, ['MSG', 'N']) as c:
    for i in range(12):
        c.insertRow(['message %d' % i, i * i])
arcpy.management.AddIndex(p, 'N', 'IX_N')
arcpy.management.ClearWorkspaceCache()
print(sorted(os.listdir(g)))
