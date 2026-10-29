"""ArcGIS bridge: testdata/geoproc/trim_lines.shp, the Trim Line fixture (WGS 1984 UTM 33N).

Groups 2 km apart: a plus (V 10/30 around H), a line crossed twice (L 20|50|30), a chain
(A crossed near its start, B touching A's end), a T (T1 touching T0's middle), an isolated
20 m line, an X of 10 m arms, a self-crossing hook, a T stub S touching M 14 m from its end
(trimming uses the original geometry), a multipart line cut by its own second part, a line
crossed twice near one end (Z). No dangle equals a case length: ArcGIS decides exact ties by
rounding noise of the crossing point.
Run: uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/research/trim_line/make_inputs.py
"""
sr = arcpy.SpatialReference(32633)
groups = [
    [('H', [[(0, 0), (100, 0)]]), ('V', [[(50, -30), (50, 10)]])],
    [('L', [[(0, 0), (100, 0)]]), ('X1', [[(20, -50), (20, 50)]]), ('X2', [[(70, -50), (70, 50)]])],
    [('A', [[(0, 0), (50, 0)]]), ('B', [[(50, 0), (80, 0)]]), ('C', [[(10, -50), (10, 50)]])],
    [('T0', [[(0, 0), (100, 0)]]), ('T1', [[(50, 0), (50, 40)]])],
    [('I', [[(0, 0), (20, 0)]])],
    [('XA', [[(10, -10), (10, 10)]]), ('XB', [[(0, 0), (20, 0)]])],
    [('J', [[(0, 0), (40, 0), (40, 20), (20, 20), (20, -8)]])],
    [('M', [[(0, 0), (60, 0)]]), ('S', [[(14, 0), (14, 20)]])],
    [('MP', [[(0, 0), (100, 0)], [(10, -30), (10, 30)]])],
    [('Z', [[(0, 0), (100, 0)]]), ('Z2', [[(5, -30), (5, 30)]]), ('Z3', [[(12, -30), (12, 30)]])],
]
folder = P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
fx = P('/mnt/media/ArcGIS_Linux/testdata/geoproc/trim_lines.shp')
arcpy.env.overwriteOutput = True
arcpy.management.CreateFeatureclass(folder, 'trim_lines.shp', 'POLYLINE', spatial_reference=sr)
arcpy.management.AddField(fx, 'NAME', 'TEXT', field_length=4)
with arcpy.da.InsertCursor(fx, ['SHAPE@', 'NAME']) as c:
    for gi, group in enumerate(groups):
        for k, parts in group:
            arr = arcpy.Array([arcpy.Array([arcpy.Point(x + 2000 * gi, y) for x, y in part]) for part in parts])
            c.insertRow((arcpy.Polyline(arr, sr), k))
print(int(arcpy.management.GetCount(fx).getOutput(0)))
