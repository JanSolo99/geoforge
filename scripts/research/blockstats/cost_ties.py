"""Searches Dijkstra variants for the one whose tie-breaks match ArcGIS's Cost Back Link.

    python3 scripts/research/blockstats/cost_ties.py
Small and bounded (a few thousand cells per run).
"""
import heapq
import itertools
import math
import os

import numpy as np
from osgeo import gdal

gdal.UseExceptions()
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
f32 = np.float32
DIRS = [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)]  # code - 1 = index


def load(path):
    return gdal.Open(os.path.join(ROOT, path)).ReadAsArray()


def run(cost, src, order, tie, update, use_f32=True):
    H, W = cost.shape
    cw = f32(30.0)
    diag = f32(30.0 * math.sqrt(2.0))
    dist = np.full((H, W), np.inf, dtype=np.float32)
    back = np.zeros((H, W), dtype=np.int8)
    pq = []
    seq = itertools.count()
    for r, c in zip(*np.nonzero((src != -9999) & (cost != -9999))):
        dist[r, c] = 0
        heapq.heappush(pq, (f32(0), tie(r * W + c, next(seq)), int(r), int(c)))
    while pq:
        d, _, r, c = heapq.heappop(pq)
        if d > dist[r, c]:
            continue
        for k in order:
            dr, dc = DIRS[k]
            rr, cc = r + dr, c + dc
            if not (0 <= rr < H and 0 <= cc < W) or cost[rr, cc] == -9999:
                continue
            step = (diag if dr and dc else cw) * ((f32(cost[r, c]) + f32(cost[rr, cc])) / f32(2))
            nd = f32(d + step)
            better = nd < dist[rr, cc] if update == "lt" else nd <= dist[rr, cc]
            if better and dist[rr, cc] != 0:
                dist[rr, cc] = nd
                back[rr, cc] = (k + 4) % 8 + 1
                heapq.heappush(pq, (nd, tie(rr * W + cc, next(seq)), rr, cc))
    return dist, back


cost = load("testdata/raster/cost_int16.tif").astype(np.float32)
src = load("testdata/raster/cost_src_i16.tif")
want = load("testdata/arcgis-reference/cback_int.tif")
cost_c = load("testdata/raster/cost_const1.tif").astype(np.float32)
src_c = load("testdata/raster/cost_src_one.tif")
want_c = load("testdata/arcgis-reference/cback_const.tif")

ties = {
    "idx asc": lambda i, s: (i, s),
    "idx desc": lambda i, s: (-i, s),
    "fifo": lambda i, s: (s, 0),
    "lifo": lambda i, s: (-s, 0),
}
results = []
for start in range(8):
    for direction in (1, -1):
        order = [(start + direction * j) % 8 for j in range(8)]
        for tname, tie in ties.items():
            for update in ("lt", "le"):
                dist, back = run(cost, src, order, tie, update)
                valid = want != 32767
                bad = int(((back != want) & valid & np.isfinite(dist) & (dist > 0)).sum())
                dist_c, back_c = run(cost_c, src_c, order, tie, update)
                valid_c = want_c != 32767
                bad_c = int(((back_c != want_c) & valid_c & np.isfinite(dist_c) & (dist_c > 0)).sum())
                results.append((bad + bad_c, bad, bad_c, start, direction, tname, update))
results.sort()
for r in results[:8]:
    print(r)
