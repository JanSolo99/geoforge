"""ArcGIS bridge: testdata/geoproc/extend_lines.shp, the Extend Line fixture (WGS 1984 UTM 33N).

Groups, far enough apart not to interact at 100 m: dangling ends near lines, a multi-vertex
end, a diagonal reaching two lines (x 0..130); self-hit, a T stub, extension crossings that
one or both ends reach, collinear ends, ray/ray meeting (x 1200..1750); a multipart line and
ends within/outside the 0.001 m tolerance and just beyond the length (y ~1000); and the
P/Q/R group in reverse OID order (x ~2500) showing sequential processing.
Run: uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/research/extend_line/make_inputs.py
"""
sr = arcpy.SpatialReference(32633)
lines = {'A': [(0, 0), (100, 0)], 'B': [(50, 5), (50, 40)], 'C': [(110, -20), (110, 20)], 'D': [(-30, 10), (-10, 10)],
         'E': [(20, -5), (20, -50)], 'F': [(60, 20), (80, 40)], 'G': [(130, 30), (130, 50)],
         'H': [(70, -30), (80, -30), (90, -10)]}
lines2 = {'J': [(200, 0), (240, 0), (240, 20), (220, 20), (220, 5)], 'K': [(300, 0), (340, 0)], 'L': [(320, 0), (320, -8)],
          'M': [(400, 0), (420, 0)], 'N': [(430, -30), (430, -20)], 'P': [(500, 0), (520, 0)], 'Q': [(530, -10), (530, 10)],
          'R': [(525, 30), (525, 15)], 'S': [(600, 0), (620, 0)], 'T': [(630, 0), (650, 0)], 'U': [(700, 0), (720, 0)],
          'V': [(730, -10), (730, -2)]}
sets = [(k, [v]) for k, v in lines.items()]
sets += [(k, [[(x + 1000, y) for x, y in v]]) for k, v in lines2.items()]
sets += [('MB', [[(0, 1000), (100, 1000)]]), ('MM', [[(10, 1005), (10, 1020)], [(30, 1005), (30, 1020)]]),
         ('T1', [[(50, 1000.0005), (50, 1020)]]), ('T2', [[(70, 1000.002), (70, 1020)]]), ('E1', [[(90, 1010.0001), (90, 1030)]])]
sets += [('R2', [[(2525, 30), (2525, 15)]]), ('P2', [[(2500, 0), (2520, 0)]]), ('Q2', [[(2530, -10), (2530, 10)]])]
folder = P('/mnt/media/ArcGIS_Linux/testdata/geoproc')
fx = P('/mnt/media/ArcGIS_Linux/testdata/geoproc/extend_lines.shp')
arcpy.env.overwriteOutput = True
arcpy.management.CreateFeatureclass(folder, 'extend_lines.shp', 'POLYLINE', spatial_reference=sr)
arcpy.management.AddField(fx, 'NAME', 'TEXT', field_length=4)
with arcpy.da.InsertCursor(fx, ['SHAPE@', 'NAME']) as c:
    for k, parts in sets:
        c.insertRow((arcpy.Polyline(arcpy.Array([arcpy.Array([arcpy.Point(*p) for p in part]) for part in parts]), sr), k))
print(int(arcpy.management.GetCount(fx).getOutput(0)))
