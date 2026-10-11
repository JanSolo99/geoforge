"""Writes testdata/gpkg/sample.gpkg with GDAL (test data only): feature tables of
each geometry type with every GeoPackage column type, Z/M variants, a polygon
with a hole, multi-part shapes, empty and null geometries, and a plain table.
shapes.gpkg holds the same without the 64-bit integer and date columns, which
ArcGIS cannot write to shapefiles.

    python3 scripts/make-gpkg-data.py
"""

from pathlib import Path

from osgeo import gdal, ogr, osr

gdal.UseExceptions()
DIR = Path(__file__).resolve().parent.parent / "testdata" / "gpkg"
drv = ogr.GetDriverByName("GPKG")
utm = osr.SpatialReference()
utm.ImportFromEPSG(32633)
wgs = osr.SpatialReference()
wgs.ImportFromEPSG(4326)


def fields(lyr, big):
    defs = [
        ("short_f", ogr.OFTInteger, ogr.OFSTInt16, 0),
        ("long_f", ogr.OFTInteger, ogr.OFSTNone, 0),
        ("big_f", ogr.OFTInteger64, ogr.OFSTNone, 0),
        ("float_f", ogr.OFTReal, ogr.OFSTFloat32, 0),
        ("double_f", ogr.OFTReal, ogr.OFSTNone, 0),
        ("text_f", ogr.OFTString, ogr.OFSTNone, 0),
        ("text20_f", ogr.OFTString, ogr.OFSTNone, 20),
        ("bool_f", ogr.OFTInteger, ogr.OFSTBoolean, 0),
        ("date_f", ogr.OFTDate, ogr.OFSTNone, 0),
        ("datetime_f", ogr.OFTDateTime, ogr.OFSTNone, 0),
        ("blob_f", ogr.OFTBinary, ogr.OFSTNone, 0),
    ]
    for name, t, sub, width in defs:
        if name in ("big_f", "date_f") and not big:
            continue
        fd = ogr.FieldDefn(name, t)
        fd.SetSubType(sub)
        if width:
            fd.SetWidth(width)
        lyr.CreateField(fd)


def add(lyr, wkt, k):
    f = ogr.Feature(lyr.GetLayerDefn())
    f.SetField("short_f", -k)
    f.SetField("long_f", 100000 * k)
    if f.GetFieldIndex("big_f") >= 0:
        f.SetField("big_f", 2**40 + k)
    f.SetField("float_f", k + 0.25)
    f.SetField("double_f", k / 3)
    f.SetField("text_f", f"text {k} é")
    f.SetField("text20_f", "x" * k)
    f.SetField("bool_f", k % 2)
    if f.GetFieldIndex("date_f") >= 0:
        f.SetField("date_f", f"2024-0{k}-1{k}")
    f.SetField("datetime_f", f"2024-0{k}-1{k}T0{k}:30:15.250Z")
    f.SetFieldBinaryFromHexString("blob_f", "00ff" * k)
    if wkt is not None:
        f.SetGeometry(ogr.CreateGeometryFromWkt(wkt))
    if k == 3:
        f.SetFieldNull("long_f")
        f.SetFieldNull("text_f")
    lyr.CreateFeature(f)


layers = {
    "points": (ogr.wkbPoint, utm, ["POINT (500100 5000100)", "POINT (500200.5 5000300.25)", None, "POINT EMPTY"]),
    "points_z": (ogr.wkbPoint25D, utm, ["POINT Z (500100 5000100 10)", "POINT Z (500200 5000300 -2.5)"]),
    "points_zm": (ogr.wkbPointZM, utm, ["POINT ZM (500100 5000100 10 1)", "POINT ZM (500200 5000300 -2.5 2)"]),
    "multipoints": (ogr.wkbMultiPoint, utm, ["MULTIPOINT ((500100 5000100),(500150 5000120))", "MULTIPOINT ((500300 5000300))"]),
    "lines": (ogr.wkbLineString, utm, ["LINESTRING (500000 5000000,500100 5000050,500200 5000000)", "LINESTRING (500300 5000300,500310 5000320)"]),
    "multilines": (ogr.wkbMultiLineString, utm, ["MULTILINESTRING ((500000 5000000,500100 5000050),(500200 5000000,500250 5000050,500300 5000000))", "MULTILINESTRING ((0 0,1 1))"]),
    "polygons": (ogr.wkbPolygon, utm, [
        "POLYGON ((500000 5000000,500100 5000000,500100 5000100,500000 5000100,500000 5000000),(500020 5000020,500020 5000040,500040 5000040,500040 5000020,500020 5000020))",
        "POLYGON ((500200 5000200,500200 5000300,500300 5000300,500200 5000200))",
    ]),
    "multipolygons": (ogr.wkbMultiPolygon, utm, [
        "MULTIPOLYGON (((500000 5000000,500100 5000000,500100 5000100,500000 5000000)),((500200 5000200,500300 5000200,500300 5000300,500200 5000300,500200 5000200),(500220 5000220,500280 5000220,500280 5000280,500220 5000280,500220 5000220)))",
        "MULTIPOLYGON (((500400 5000400,500500 5000400,500500 5000500,500400 5000400)))",
    ]),
    "polygons_z": (ogr.wkbPolygon25D, utm, ["POLYGON Z ((500000 5000000 1,500100 5000000 2,500100 5000100 3,500000 5000000 1))"]),
    "geographic": (ogr.wkbPoint, wgs, ["POINT (15.5 47.25)", "POINT (-120.125 -33.5)"]),
}
for file, big in [("sample.gpkg", True), ("shapes.gpkg", False)]:
    (DIR / file).unlink(missing_ok=True)
    ds = drv.CreateDataSource(str(DIR / file))
    for name, (gtype, srs, wkts) in layers.items():
        lyr = ds.CreateLayer(name, srs, gtype)
        fields(lyr, big)
        for k, wkt in enumerate(wkts, start=1):
            add(lyr, wkt, k)
    table = ds.CreateLayer("attributes", None, ogr.wkbNone)
    fields(table, big)
    for k in range(1, 4):
        add(table, None, k)
    ds = None
