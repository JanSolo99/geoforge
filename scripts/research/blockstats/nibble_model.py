"""Models Nibble against the reference rasters (nibble_*).

    python3 scripts/research/blockstats/nibble_model.py
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


for case, info in sorted(manifest.items()):
    if not case.startswith("nibble_"):
        continue
    a, nd, dt = read(info["args"][0])
    m, mnd, _ = read(info["args"][1])
    out, ond, odt = read("testdata/arcgis-reference/" + case + ".tif")
    print(case, info["args"][3:], dt, "->", odt, "nodata", ond)
    masked = m == mnd
    valid_out = out != ond
    print("   masked cells", int(masked.sum()), "output valid", int(valid_out.sum()), "input nodata", int((a == nd).sum()) if nd is not None else None)
    # unmasked cells keep their value?
    keep = ~masked
    print("   unmasked equal to input", int((out[keep] == a[keep]).sum()), "of", int(keep.sum()))
    # masked cells: nearest unmasked
    pts = np.argwhere(keep & ((a != nd) if nd is not None else True))
    res = {"nearest-first": 0, "ties": 0}
    bad = 0
    tie_cells = 0
    for r, c in np.argwhere(masked):
        d2 = (pts[:, 0] - r) ** 2 + (pts[:, 1] - c) ** 2
        best = d2.min()
        cand = pts[d2 == best]
        vals = {int(a[tuple(p)]) for p in cand}
        got = int(out[r, c]) if valid_out[r, c] else None
        if got not in vals:
            bad += 1
        if len(vals) > 1:
            tie_cells += 1
    print("   masked cells whose value is not a nearest neighbour's:", bad, "tie cells:", tie_cells)
