"""Creates testdata/dm/chain.shp with ArcGIS (run: uv run -q --with "mcp>=2" python
tools/arcgis-bridge/run_code.py scripts/make-dm-chain.py): lines that continue each other, a T
junction, a loop and a multipart line, for Unsplit Line, Split Line At Vertices and friends."""
import arcpy
arcpy.env.overwriteOutput = True
out = '/mnt/media/ArcGIS_Linux/testdata/dm'
fc = arcpy.management.CreateFeatureclass(P(out), 'chain.shp', 'POLYLINE', spatial_reference=arcpy.SpatialReference(32632))
for n, t, l in [('LINE_ID', 'LONG', None), ('KIND', 'TEXT', 10), ('CLASS', 'SHORT', None)]:
    arcpy.management.AddField(fc, n, t, field_length=l) if l else arcpy.management.AddField(fc, n, t)
rows = [
    (1, 'a', 1, [[(500000, 5000000), (500100, 5000000), (500100, 5000100)]]),
    (2, 'a', 1, [[(500100, 5000100), (500200, 5000100)]]),
    (3, 'a', 2, [[(500300, 5000100), (500200, 5000100)]]),
    (4, 'b', 1, [[(500300, 5000100), (500300, 5000200)]]),
    (5, 'b', 1, [[(500300, 5000200), (500400, 5000250)]]),
    (6, 'b', 1, [[(500400, 5000250), (500500, 5000300)]]),
    (7, 'c', 1, [[(500400, 5000250), (500500, 5000200)]]),
    (8, 'c', 1, [[(500000, 5000300), (500100, 5000300), (500100, 5000400), (500000, 5000300)]]),
    (9, 'c', 1, [[(500600, 5000000), (500700, 5000000)], [(500700, 5000000), (500700, 5000100)]]),
    (10, 'c', 1, [[(500600, 5000300), (500650, 5000320), (500700, 5000300), (500750, 5000350)]]),
]
with arcpy.da.InsertCursor(fc, ['SHAPE@', 'LINE_ID', 'KIND', 'CLASS']) as c:
    for i, k, cl, parts in rows:
        arr = arcpy.Array([arcpy.Array([arcpy.Point(*p) for p in part]) for part in parts])
        c.insertRow([arcpy.Polyline(arr), i, k, cl])
print(arcpy.management.GetCount(fc))

# Points for Split Line At Point: on a line, at a vertex, 5 m off a line, at a junction, two on one
# line (given out of order), a far one.
pts = arcpy.management.CreateFeatureclass(P(out), 'pts.shp', 'POINT', spatial_reference=arcpy.SpatialReference(32632))
arcpy.management.AddField(pts, 'PT_ID', 'LONG')
with arcpy.da.InsertCursor(pts, ['SHAPE@XY', 'PT_ID']) as c:
    for i, xy in enumerate([(500050, 5000000), (500100, 5000000), (500150, 5000105), (500200, 5000100),
                            (500300, 5000180), (500300, 5000120), (500650, 5000300.5), (500900, 5000900)], 1):
        c.insertRow([xy, i])

# Polygons for Eliminate Polygon Part: holes of several sizes and small and large extra parts.
def ring(x, y, w, h):
    return [(x, y), (x, y + h), (x + w, y + h), (x + w, y), (x, y)]
polys = arcpy.management.CreateFeatureclass(P(out), 'parts.shp', 'POLYGON', spatial_reference=arcpy.SpatialReference(32632))
arcpy.management.AddField(polys, 'POLY_ID', 'LONG')
shapes = [
    # 100 x 100 with holes of 400, 25 and 2500 m2, and islands of 100 and 900 m2 beside it
    (1, [ring(500000, 5000000, 100, 100), ring(500010, 5000010, 20, 20)[::-1], ring(500050, 5000010, 5, 5)[::-1],
         ring(500040, 5000040, 50, 50)[::-1], ring(500200, 5000000, 10, 10), ring(500300, 5000000, 30, 30)]),
    (2, [ring(500000, 5000200, 60, 60)]),
    (3, [ring(500200, 5000200, 40, 40), ring(500300, 5000200, 40, 40), ring(500400, 5000200, 5, 5)]),
]
with arcpy.da.InsertCursor(polys, ['SHAPE@', 'POLY_ID']) as c:
    for i, rings in shapes:
        c.insertRow([arcpy.Polygon(arcpy.Array([arcpy.Array([arcpy.Point(*p) for p in r]) for r in rings])), i])

# Points without a coordinate system, for Define Projection.
nop = arcpy.management.CreateFeatureclass(P(out), 'noprj.shp', 'POINT')
arcpy.management.AddField(nop, 'PT_ID', 'LONG')
with arcpy.da.InsertCursor(nop, ['SHAPE@XY', 'PT_ID']) as c:
    for i, xy in enumerate([(500050, 5000000), (500100, 5000100), (500200, 5000200)], 1):
        c.insertRow([xy, i])

# Points with a start and an end date, for Calculate End Time.
import datetime as dt
ev = arcpy.management.CreateFeatureclass(P(out), 'events.shp', 'POINT', spatial_reference=arcpy.SpatialReference(32632))
for n, t, l in [('NAME', 'TEXT', 10), ('STARTD', 'DATE', None), ('ENDD', 'DATE', None), ('GRP', 'LONG', None)]:
    arcpy.management.AddField(ev, n, t, field_length=l) if l else arcpy.management.AddField(ev, n, t)
rows = [('a', dt.datetime(2020, 1, 1), 1), ('a', dt.datetime(2020, 3, 1), 1), ('b', dt.datetime(2020, 2, 1), 2),
        ('a', dt.datetime(2020, 2, 15), 1), ('b', dt.datetime(2020, 5, 1), 2), ('c', dt.datetime(2021, 1, 1), 3),
        ('a', dt.datetime(2020, 2, 15), 2), ('b', None, 2)]
with arcpy.da.InsertCursor(ev, ['SHAPE@XY', 'NAME', 'STARTD', 'GRP']) as c:
    for i, (n, d, g) in enumerate(rows):
        c.insertRow([(500000 + i * 10, 5000000), n, d, g])
