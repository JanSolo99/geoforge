"""Compares ArcGIS Block Statistics outputs (testdata/arcgis-reference/blk_*.tif) with a model.

    python3 scripts/research/blockstats/analyze.py
"""
import glob
import json
import os
import re

import numpy as np
from osgeo import gdal

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
ref = os.path.join(ROOT, "testdata/arcgis-reference")
manifest = json.load(open(os.path.join(ref, "manifest.json")))["cases"]


def read(path):
    ds = gdal.Open(path)
    b = ds.GetRasterBand(1)
    return b.ReadAsArray(), b.GetNoDataValue(), gdal.GetDataTypeName(b.DataType)


def counts(v):
    vals, cnt = np.unique(v, return_counts=True)
    return vals, cnt


def stat(name, v, center=None):
    if name == "MEAN":
        return v.mean()
    if name == "SUM":
        return v.sum()
    if name == "MAXIMUM":
        return v.max()
    if name == "MINIMUM":
        return v.min()
    if name == "RANGE":
        return v.max() - v.min()
    if name == "STD":
        return v.std()
    if name == "MEDIAN":
        # integer data: the lower of the two middle values
        return np.sort(v)[(len(v) - 1) // 2]
    vals, cnt = counts(v)
    if name == "VARIETY":
        return len(vals)
    best = cnt.max() if name == "MAJORITY" else cnt.min()
    tied = vals[cnt == best]
    return tied[0]


for case, info in sorted(manifest.items()):
    if not case.startswith("blk_"):
        continue
    args = info["args"]
    src = os.path.join(ROOT, args[0])
    nbr, st, ign = args[2], args[3], args[4]
    a, nd, dt = read(src)
    out, ond, odt = read(os.path.join(ref, case + ".tif"))
    print(f"{case:24s} in {dt:8s} out {odt:8s} nodata {ond}")


def blocks_rect(shape, bh, bw):
    for i in range(0, shape[0], bh):
        for j in range(0, shape[1], bw):
            yield i, j, min(i + bh, shape[0]), min(j + bw, shape[1])


print()
for case, info in sorted(manifest.items()):
    if not case.startswith("blk_"):
        continue
    args = info["args"]
    src = os.path.join(ROOT, args[0])
    nbr, st, ign = args[2], args[3], args[4]
    if not nbr.startswith("Rectangle") or nbr.endswith("MAP"):
        continue
    w, h = (int(x) for x in nbr.split()[1:3])
    a, nd, dt = read(src)
    out, ond, odt = read(os.path.join(ref, case + ".tif"))
    a = a.astype(float)
    exp = np.full(a.shape, np.nan)
    for i0, j0, i1, j1 in blocks_rect(a.shape, h, w):
        blk = a[i0:i1, j0:j1]
        mask = np.ones(blk.shape, bool) if nd is None else blk != nd
        if ign == "NODATA" and (not mask.all() or blk.shape != (h, w)):
            continue
        v = blk[mask]
        if v.size:
            exp[i0:i1, j0:j1] = stat(st, v)
    if odt not in ("Float32", "Float64"):
        # integer output: ArcGIS stores trunc(v + 0.5)
        exp = np.where(np.isnan(exp), exp, np.trunc(exp + 0.5))
    valid = out.astype(float) != ond
    bad = ~np.isclose(np.where(valid, out, np.nan), exp, equal_nan=True, rtol=1e-6, atol=1e-6)
    print(f"{case:24s} {st:8s} cells differing {bad.sum():4d} of {a.size}")
