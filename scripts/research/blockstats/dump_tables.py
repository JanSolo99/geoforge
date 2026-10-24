"""Prints the dbf tables of reference cases: python3 dump_tables.py CASE [CASE ...] [--rows N]"""
import sys

from osgeo import ogr

ogr.UseExceptions()
rows = 8
args = sys.argv[1:]
if "--rows" in args:
    i = args.index("--rows")
    rows = int(args[i + 1])
    del args[i : i + 2]
for case in args:
    ds = ogr.Open(f"testdata/arcgis-reference/{case}.dbf")
    lyr = ds.GetLayer(0)
    d = lyr.GetLayerDefn()
    print("==", case, lyr.GetFeatureCount(), "rows")
    print("  ", [(d.GetFieldDefn(i).GetName(), d.GetFieldDefn(i).GetTypeName(), d.GetFieldDefn(i).GetWidth(), d.GetFieldDefn(i).GetPrecision()) for i in range(d.GetFieldCount())])
    for n, f in enumerate(lyr):
        if n >= rows:
            break
        print("  ", [f.GetField(i) for i in range(d.GetFieldCount())])
