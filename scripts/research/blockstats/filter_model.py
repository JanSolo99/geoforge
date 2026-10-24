"""Models Spatial Analyst Filter against the reference rasters (filter_*).

    python3 scripts/research/blockstats/filter_model.py
"""
import json
import os

import numpy as np
from osgeo import gdal

gdal.UseExceptions()
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
manifest = json.load(open(os.path.join(ROOT, "testdata/arcgis-reference/manifest.json")))["cases"]


def read(path):
    ds = gdal.Open(os.path.join(ROOT, path))
    b = ds.GetRasterBand(1)
    return b.ReadAsArray(), b.GetNoDataValue(), gdal.GetDataTypeName(b.DataType)


LOW = np.ones((3, 3)) / 9.0
HIGH_CANDIDATES = {
    "-1/8": np.array([[-1, -1, -1], [-1, 8, -1], [-1, -1, -1]], float),
    "-0.7/-1/6.8": np.array([[-0.7, -1, -0.7], [-1, 6.8, -1], [-0.7, -1, -0.7]], float),
}


def apply(a, nd, kernel, ignore, edge):
    H, W = a.shape
    out = np.full(a.shape, np.nan)
    for r in range(H):
        for c in range(W):
            if nd is not None and a[r, c] == nd:
                continue
            acc, wsum, ok = 0.0, 0.0, True
            for i in range(3):
                for j in range(3):
                    rr, cc = r + i - 1, c + j - 1
                    inside = 0 <= rr < H and 0 <= cc < W
                    v = a[rr, cc] if inside else None
                    if v is None or (nd is not None and v == nd):
                        if ignore:
                            continue
                        ok = False
                        continue
                    acc += kernel[i, j] * v
                    wsum += kernel[i, j]
            if not ok:
                continue
            if edge == "renorm" and ignore:
                full = kernel.sum()
                if abs(wsum) > 1e-12 and abs(full) > 1e-12:
                    acc = acc * full / wsum
            out[r, c] = acc
    return out


for case, info in sorted(manifest.items()):
    if not case.startswith("filter_"):
        continue
    a, nd, dt = read(info["args"][0])
    out, ond, odt = read("testdata/arcgis-reference/" + case + ".tif")
    kind, ignore = info["args"][2], info["args"][3] == "DATA"
    valid = out != ond
    print(case, dt, "->", odt, "nodata", ond, "valid", int(valid.sum()), "of", out.size)
    kernels = {"low": LOW} if kind == "LOW" else HIGH_CANDIDATES
    for name, k in kernels.items():
        for edge in ("plain", "renorm"):
            exp = apply(a.astype(float), nd, k, ignore, edge)
            both = valid & ~np.isnan(exp)
            mism = int((valid != ~np.isnan(exp)).sum())
            diff = float(np.abs(out[both] - exp[both]).max()) if both.any() else -1
            print(f"   {name:12s} {edge:6s} validity mismatch {mism:4d} maxdiff {diff:.5f}")
