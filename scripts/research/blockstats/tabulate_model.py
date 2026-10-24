"""Models Tabulate Area's processing grid against the ArcGIS reference tables.

    python3 scripts/research/blockstats/tabulate_model.py
"""
import json
import math
import os

import numpy as np
from osgeo import gdal, ogr

gdal.UseExceptions()
ogr.UseExceptions()
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
manifest = json.load(open(os.path.join(ROOT, "testdata/arcgis-reference/manifest.json")))["cases"]


def raster(path):
    ds = gdal.Open(os.path.join(ROOT, path))
    b = ds.GetRasterBand(1)
    gt = ds.GetGeoTransform()
    return b.ReadAsArray(), b.GetNoDataValue(), gt


def table(case):
    ds = ogr.Open(os.path.join(ROOT, f"testdata/arcgis-reference/{case}.dbf"))
    lyr = ds.GetLayer(0)
    d = lyr.GetLayerDefn()
    names = [d.GetFieldDefn(i).GetName() for i in range(d.GetFieldCount())]
    return names, [[f.GetField(i) for i in range(len(names))] for f in lyr]


def sample(r, x, y):
    a, nd, gt = r
    col = math.floor((x - gt[0]) / gt[1])
    row = math.floor((y - gt[3]) / gt[5])
    if 0 <= row < a.shape[0] and 0 <= col < a.shape[1]:
        v = a[row, col]
        return None if (nd is not None and v == nd) else int(v)
    return None


def tabulate(z, c, cell, extent_rule, origin_rule):
    (_, _, zg), (_, _, cg) = z, c
    ze = (zg[0], zg[3] + zg[5] * z[0].shape[0], zg[0] + zg[1] * z[0].shape[1], zg[3])
    ce = (cg[0], cg[3] + cg[5] * c[0].shape[0], cg[0] + cg[1] * c[0].shape[1], cg[3])
    if extent_rule == "intersect":
        e = (max(ze[0], ce[0]), max(ze[1], ce[1]), min(ze[2], ce[2]), min(ze[3], ce[3]))
    elif extent_rule == "zone":
        e = ze
    elif extent_rule == "class":
        e = ce
    else:
        e = (min(ze[0], ce[0]), min(ze[1], ce[1]), max(ze[2], ce[2]), max(ze[3], ce[3]))
    ox, oy = (e[0], e[3])
    if origin_rule == "zone":
        ox, oy = zg[0] + math.floor((e[0] - zg[0]) / cell) * cell, zg[3] - math.floor((zg[3] - e[3]) / cell) * cell
    elif origin_rule == "class":
        ox, oy = cg[0] + math.floor((e[0] - cg[0]) / cell) * cell, cg[3] - math.floor((cg[3] - e[3]) / cell) * cell
    cols = int(math.ceil((e[2] - ox) / cell - 1e-9))
    rows = int(math.ceil((oy - e[1]) / cell - 1e-9))
    out = {}
    for r in range(rows):
        for k in range(cols):
            x, y = ox + (k + 0.5) * cell, oy - (r + 0.5) * cell
            zv, cv = sample(z, x, y), sample(c, x, y)
            if zv is None or cv is None:
                continue
            out[(zv, cv)] = out.get((zv, cv), 0) + cell * cell
    return out


for case in ("ta_i32_i16", "ta_i32_i16_cell60", "ta_i32_i16_cell20", "ta_zones25_landuse", "ta_landuse_zones4"):
    args = manifest[case]["args"]
    z, c = raster(args[0]), raster(args[2])
    names, rows = table(case)
    want = {}
    for row in rows:
        for n, v in zip(names[1:], row[1:]):
            want[(row[0], int(n.split("_")[1]))] = v
    cell = float(args[5]) if len(args) > 5 and args[5] not in ("", None) else max(z[2][1], c[2][1])
    res = []
    for ext in ("intersect", "zone", "class", "union"):
        for org in ("extent", "zone", "class"):
            got = tabulate(z, c, cell, ext, org)
            bad = sum(1 for k, v in want.items() if abs(got.get(k, 0.0) - v) > 1e-6)
            res.append((bad, ext, org))
    res.sort()
    print(case, "cell", cell, "best", res[:3])


def grid_search(case):
    args = manifest[case]["args"]
    z, c = raster(args[0]), raster(args[2])
    names, rows = table(case)
    want = {}
    for row in rows:
        for n, v in zip(names[1:], row[1:]):
            want[(row[0], int(n.split("_")[1]))] = v
    cell = max(z[2][1], c[2][1])
    best = []
    zg, cg = z[2], c[2]
    for sx in range(0, int(cell)):
        for sy in range(0, int(cell)):
            ox, oy = zg[0] + sx - cell, zg[3] - sy + cell
            out = {}
            for r in range(int(1400 / cell) + 4):
                for k in range(int(1700 / cell) + 4):
                    x, y = ox + (k + 0.5) * cell, oy - (r + 0.5) * cell
                    zv, cv = sample(z, x, y), sample(c, x, y)
                    if zv is None or cv is None:
                        continue
                    out[(zv, cv)] = out.get((zv, cv), 0) + cell * cell
            bad = sum(1 for k, v in want.items() if abs(out.get(k, 0.0) - v) > 1e-6)
            best.append((bad, sx, sy))
    best.sort()
    print(case, best[:5])


grid_search("ta_landuse_zones4")


def frac_search(case):
    args = manifest[case]["args"]
    z, c = raster(args[0]), raster(args[2])
    names, rows = table(case)
    want = {}
    for row in rows:
        for n, v in zip(names[1:], row[1:]):
            want[(row[0], int(n.split("_")[1]))] = v
    cell = 30.0
    zg = z[2]
    res = []
    for fx in (0, 0.25, 0.5, 0.75, 1):
        for fy in (0, 0.25, 0.5, 0.75, 1):
            out = {}
            for r in range(40):
                for k in range(50):
                    x, y = zg[0] + (k + fx) * cell, zg[3] - (r + fy) * cell
                    zv = sample(z, x - 1e-6 * (fx == 1), y + 1e-6 * (fy == 1))
                    cv = sample(c, x, y)
                    if zv is None or cv is None:
                        continue
                    out[(zv, cv)] = out.get((zv, cv), 0) + cell * cell
            bad = sum(1 for k, v in want.items() if abs(out.get(k, 0.0) - v) > 1e-6)
            res.append((bad, fx, fy))
    res.sort()
    print("frac", case, res[:4])


frac_search("ta_landuse_zones4")


def zh_search(case, anchor):
    args = manifest[case]["args"]
    z, c = raster(args[0]), raster(args[2])
    names, rows = table(case)
    want = {}
    for row in rows:
        for n, v in zip(names[1:], row[1:]):
            tail = n.split("_", 1)[1]
            if tail.startswith("_"):
                continue   # a negative zone: the name was cut
            want[(int(tail), int(row[0]))] = v   # (zone, value)
    cell = max(z[2][1], c[2][1])
    g = (z if anchor == "zone" else c)[2]
    shape = (z if anchor == "zone" else c)[0].shape
    res = []
    for fx in (0, 0.25, 0.5, 0.75, 1):
        for fy in (0, 0.25, 0.5, 0.75, 1):
            out = {}
            for r in range(int(shape[0] * g[1] / cell) + 2):
                for k in range(int(shape[1] * g[1] / cell) + 2):
                    x, y = g[0] + (k + fx) * cell, g[3] - (r + fy) * cell
                    zv, cv = sample(z, x - 1e-6 * (fx == 1), y + 1e-6 * (fy == 1)), sample(c, x, y)
                    if zv is None or cv is None:
                        continue
                    out[(zv, cv)] = out.get((zv, cv), 0) + 1
            bad = sum(1 for k, v in want.items() if out.get(k, 0) != v)
            res.append((bad, fx, fy))
    res.sort()
    print("zh", case, anchor, res[:4])


for case, anchor in (("zh_zones25_landuse", "class"), ("zh_zones25_landuse", "zone"), ("zh_landuse_zones4", "zone"), ("zh_landuse_zones4", "class"), ("zh_i32_i16", "zone"), ("zh_classes_landuse", "zone")):
    zh_search(case, anchor)
