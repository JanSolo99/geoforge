"""ArcGIS bridge: cga_{parcels,roads,wells}.shp, copies of the geoproc fixtures with empty
target fields for Calculate Geometry Attributes.

Run: uv run -q --with "mcp>=2" python tools/arcgis-bridge/run_code.py scripts/research/calculate_geometry_attributes/make_inputs.py
"""
D = '/mnt/media/ArcGIS_Linux/testdata/geoproc'
arcpy.env.overwriteOutput = True
spec = {
    'parcels': [('AREA_D', 'DOUBLE'), ('PERIM_D', 'DOUBLE'), ('CX', 'DOUBLE'), ('CY', 'DOUBLE'), ('IX', 'DOUBLE'),
                ('IY', 'DOUBLE'), ('XMIN', 'DOUBLE'), ('YMAX', 'DOUBLE'), ('PARTS', 'LONG'), ('PNTS', 'LONG'),
                ('HOLES', 'SHORT'), ('AREA_L', 'LONG')],
    'roads': [('LEN_D', 'DOUBLE'), ('SX', 'DOUBLE'), ('SY', 'DOUBLE'), ('EX', 'DOUBLE'), ('EY', 'DOUBLE'),
              ('BEAR', 'DOUBLE'), ('PNTS', 'LONG')],
    'wells': [('PX', 'DOUBLE'), ('PY', 'DOUBLE')],
}
for name, fields in spec.items():
    out = P(f'{D}/cga_{name}.shp')
    arcpy.management.CopyFeatures(P(f'{D}/{name}.shp'), out)
    for f, k in fields:
        arcpy.management.AddField(out, f, k)
    print(name, [f.name for f in arcpy.ListFields(out)])
