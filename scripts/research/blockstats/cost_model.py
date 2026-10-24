"""Models the cost-distance family against the ArcGIS reference rasters.

    python3 scripts/research/blockstats/cost_model.py
"""
import heapq
import json
import math
import os

import numpy as np
from osgeo import gdal

gdal.UseExceptions()
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
manifest = json.load(open(os.path.join(ROOT, "testdata/arcgis-reference/manifest.json")))["cases"]


def read(path):
    ds = gdal.Open(os.path.join(ROOT, path))
    b = ds.GetRasterBand(1)
    return b.ReadAsArray(), b.GetNoDataValue(), gdal.GetDataTypeName(b.DataType), ds.GetGeoTransform()


# neighbour order and the backlink code of the move from a cell to the neighbour *it came from*
DIRS = [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)]  # (drow, dcol): E SE S SW W NW N NE


def dijkstra(cost, cnd, src, snd, cell, maxd=None):
    H, W = cost.shape
    dist = np.full((H, W), np.inf)
    label = np.full((H, W), -1, dtype=np.int64)
    back = np.full((H, W), -1, dtype=np.int64)
    pq = []
    for r in range(H):
        for c in range(W):
            if src[r, c] != snd and cost[r, c] != cnd:
                dist[r, c] = 0.0
                label[r, c] = src[r, c]
                back[r, c] = 0
                heapq.heappush(pq, (0.0, r, c))
    while pq:
        d, r, c = heapq.heappop(pq)
        if d > dist[r, c]:
            continue
        for k, (dr, dc) in enumerate(DIRS):
            rr, cc = r + dr, c + dc
            if not (0 <= rr < H and 0 <= cc < W) or cost[rr, cc] == cnd:
                continue
            step = cell * (math.sqrt(2) if dr and dc else 1.0) * (float(cost[r, c]) + float(cost[rr, cc])) / 2
            nd_ = d + step
            if maxd is not None and nd_ > maxd:
                continue
            if nd_ < dist[rr, cc]:
                dist[rr, cc] = nd_
                label[rr, cc] = label[r, c]
                back[rr, cc] = k
                heapq.heappush(pq, (nd_, rr, cc))
    return dist, label, back


for case, info in sorted(manifest.items()):
    if not case.startswith(("cdist_", "cback_", "calloc_")):
        continue
    a = info["args"]
    src, snd, _, gt = read(a[0])
    cost, cnd, _, _ = read(a[1])
    maxd = float(a[3]) if len(a) > 3 and a[3] not in ("", None) else None
    out, ond, odt, _ = read(case + ".tif" if False else "testdata/arcgis-reference/" + case + ".tif")
    dist, label, back = dijkstra(cost, cnd, src, snd, gt[1], maxd)
    valid = out != ond
    if case.startswith("cdist"):
        m = np.isfinite(dist)
        bad = (valid != m).sum() + (np.abs(np.where(valid & m, out - dist, 0)) > 1e-3).sum()
        print(f"{case:16s} {odt:8s} nodata {ond}  cells differing {bad}")
    elif case.startswith("calloc"):
        m = np.isfinite(dist)
        bad = (valid != m).sum() + ((out != label) & valid & m).sum()
        print(f"{case:16s} {odt:8s} nodata {ond}  cells differing {bad}")
    else:
        # backlink codes: find the mapping from direction index to code by majority
        m = np.isfinite(dist)
        codes = {}
        for k in range(-1, 8):
            sel = valid & m & (back == k)
            if sel.any():
                vals, cnt = np.unique(out[sel], return_counts=True)
                codes[k] = dict(zip(vals.tolist(), cnt.tolist()))
        print(f"{case:16s} {odt:8s} nodata {ond}  codes by model direction {codes}")


CODE = {0: 5, 1: 6, 2: 7, 3: 8, 4: 1, 5: 2, 6: 3, 7: 4}


def dijkstra_variant(cost, cnd, src, snd, cell, maxd, order, update, heapkey, f32):
    H, W = cost.shape
    dist = np.full((H, W), np.inf)
    back = np.full((H, W), -1, dtype=np.int64)
    pq = []
    for r in range(H):
        for c in range(W):
            if src[r, c] != snd and cost[r, c] != cnd:
                dist[r, c] = 0.0
                back[r, c] = 0
                heapq.heappush(pq, (heapkey(0.0, r, c, W), r, c))
    dirs = [DIRS[i] for i in order]
    idx = [i for i in order]
    while pq:
        key, r, c = heapq.heappop(pq)
        d = dist[r, c]
        if key[0] != d if isinstance(key, tuple) else key != d:
            pass
        for k, (dr, dc) in zip(idx, dirs):
            rr, cc = r + dr, c + dc
            if not (0 <= rr < H and 0 <= cc < W) or cost[rr, cc] == cnd:
                continue
            step = cell * (math.sqrt(2) if dr and dc else 1.0) * (float(cost[r, c]) + float(cost[rr, cc])) / 2
            nd_ = d + step
            if f32:
                nd_ = float(np.float32(nd_))
            if maxd is not None and nd_ > maxd:
                continue
            better = nd_ < dist[rr, cc] if update == "lt" else nd_ <= dist[rr, cc]
            if better and not (update == "le" and back[rr, cc] == 0):
                changed = nd_ != dist[rr, cc]
                dist[rr, cc] = nd_
                back[rr, cc] = (k + 4) % 8 if False else k
                heapq.heappush(pq, (heapkey(nd_, rr, cc, W), rr, cc))
    return dist, back


def variants():
    orders = {
        "ESWN-cw": [0, 1, 2, 3, 4, 5, 6, 7],
        "NESW-cw": [6, 7, 0, 1, 2, 3, 4, 5],
        "straight-first": [0, 2, 4, 6, 1, 3, 5, 7],
        "straight-first-N": [6, 0, 2, 4, 1, 3, 5, 7],
        "diag-first": [1, 3, 5, 7, 0, 2, 4, 6],
        "ccw": [0, 7, 6, 5, 4, 3, 2, 1],
    }
    keys = {
        "dist,r,c": lambda d, r, c, W: (d, r, c),
        "dist,c,r": lambda d, r, c, W: (d, c, r),
        "dist,-r,-c": lambda d, r, c, W: (d, -r, -c),
        "dist,fifo": None,
    }
    return orders, keys


def try_variants():
    orders, keys = variants()
    for case in ("cback_const", "cback_int", "cback_basic"):
        a = manifest[case]["args"]
        src, snd, _, gt = read(a[0])
        cost, cnd, _, _ = read(a[1])
        out, ond, _, _ = read("testdata/arcgis-reference/" + case + ".tif")
        valid = out != ond
        res = []
        for oname, order in orders.items():
            for update in ("lt", "le"):
                for kname, key in keys.items():
                    if key is None:
                        counter = [0]

                        def key(d, r, c, W, counter=counter):
                            counter[0] += 1
                            return (d, counter[0])
                    for f32 in (False, True):
                        dist, back = dijkstra_variant(cost, cnd, src, snd, gt[1], None, order, update, key, f32)
                        m = np.isfinite(dist)
                        codes = np.vectorize(lambda k: CODE.get(int(k), 0) if k >= 0 else -1)(back)
                        codes = np.where(back == 0, np.where(dist == 0, 0, codes), codes)
                        # sources are 0
                        codes = np.where(dist == 0, 0, codes)
                        bad = int(((codes != out) & valid & m).sum())
                        res.append((bad, oname, update, kname, f32))
        res.sort()
        print(case, res[:4])


try_variants()
