"""ArcGIS bridge: boundary, node, overlap, hole and multipart controls."""
arcpy.env.overwriteOutput = True
folder = P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
sr = arcpy.SpatialReference(32633)
path = folder + '/pairwise_neighbors.shp'
arcpy.management.CreateFeatureclass(folder, 'pairwise_neighbors.shp', 'POLYGON', spatial_reference=sr)
arcpy.management.AddField(path, 'NAME', 'TEXT', field_length=8)
arcpy.management.AddField(path, 'GRP', 'LONG')
def rect(x, y, w, h):
    return [(x,y),(x,y+h),(x+w,y+h),(x+w,y),(x,y)]
rows = [
 ('A',1,[rect(0,0,10,10)]), ('B',1,[rect(10,0,10,10)]),
 ('C',2,[rect(20,10,10,10)]), ('D',2,[rect(5,5,10,10)]),
 ('E',3,[rect(2,2,2,2)]), ('F',3,[rect(100,0,10,10)]),
 ('G',4,[rect(0,0,10,10)]),
 ('H',5,[rect(40,0,20,20),list(reversed(rect(45,5,10,10)))]),
 ('I',6,[rect(45,5,10,10)]), ('J',7,[rect(47,7,2,2)]),
 ('K',1,[rect(70,0,5,5),rect(80,0,5,5)]), ('L',2,[rect(75,0,5,5)]),
]
with arcpy.da.InsertCursor(path, ['SHAPE@','NAME','GRP']) as cursor:
 for name, group, rings in rows:
  shape = arcpy.Polygon(arcpy.Array([arcpy.Array([arcpy.Point(500000+x,5000000+y) for x,y in ring]) for ring in rings]),sr)
  cursor.insertRow((shape,name,group))
print('Created12 polygon controls')
