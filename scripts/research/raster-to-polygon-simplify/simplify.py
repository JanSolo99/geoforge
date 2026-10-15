"""Black-box model of ArcGIS Pro RasterToPolygon(simplify="SIMPLIFY").

simplify_polygons(raster_array, cellsize, origin, nodata=None) ->
    list of {'value': v, 'rings': [ring, ...]}   (rings closed, map coordinates,
    outer ring clockwise first, holes after), one entry per 4-connected region in
    raster order of the region's first cell.

origin = (x_left, y_top) of the raster (upper-left corner), rows go down.
See notes.md for the rule.
"""
import math
import numpy as np

KMAX = 32
DEBUG = False
FORCE = None
MODE = 'grid'
TIE = 'both'
CHORD = 'disp_orig'
SPLIT = 'le'
TOL = 0.5
EPS = 1e-7


def a_of(k):
    return 0.49 - k * 0.29 / 32


# ---------------------------------------------------------------- geometry
def dist_line(p, a, b):
    dx, dy = b[0] - a[0], b[1] - a[1]
    L = math.hypot(dx, dy)
    if L < 1e-12:
        return math.hypot(p[0] - a[0], p[1] - a[1])
    return abs(dx * (p[1] - a[1]) - dy * (p[0] - a[0])) / L


def inter(p1, p2, p3, p4):
    x1, y1 = p1; x2, y2 = p2; x3, y3 = p3; x4, y4 = p4
    d = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4)
    if abs(d) < 1e-15:
        return None
    t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / d
    return (x1 + t * (x2 - x1), y1 + t * (y2 - y1))


def _pts_to_poly(P, O):
    A = O[:-1]; B = O[1:]; D = B - A
    L2 = (D * D).sum(1); L2[L2 == 0] = 1
    t = ((P[:, None, :] - A[None]) * D[None]).sum(2) / L2[None]
    t = np.clip(t, 0, 1)
    C = A[None] + t[..., None] * D[None]
    return np.sqrt(((P[:, None, :] - C) ** 2).sum(2)).min(1)


def seg_dev(p, q, chain, t0=0.0):
    """max over the segment p-q (parameter t in [t0, 1-t0]) of the distance to polyline `chain`."""
    O = np.asarray(chain, float)
    n = max(3, int(math.dist(p, q) / 0.02) + 2)
    t = np.linspace(t0, 1 - t0, n)
    P = np.array(p)[None] * (1 - t[:, None]) + np.array(q)[None] * t[:, None]
    d = _pts_to_poly(P, O)
    i = int(d.argmax())
    lo, hi = t[max(0, i - 1)], t[min(n - 1, i + 1)]
    # refine (distance along segment is piecewise smooth; golden search near max)
    best = d[i]
    for _ in range(40):
        m1 = lo + (hi - lo) * 0.382; m2 = lo + (hi - lo) * 0.618
        P2 = np.array([[p[0] + (q[0] - p[0]) * m, p[1] + (q[1] - p[1]) * m] for m in (m1, m2)])
        d1, d2 = _pts_to_poly(P2, O)
        best = max(best, d1, d2)
        if d1 > d2:
            hi = m2
        else:
            lo = m1
    return float(best)


def turnsign(ch, i):
    n = len(ch)
    p, q, r = ch[(i - 1) % n], ch[i], ch[(i + 1) % n]
    return (q[0] - p[0]) * (r[1] - q[1]) - (q[1] - p[1]) * (r[0] - q[0])


def displace(chain, a, closed):
    """Move each interior corner diagonally by a toward the inside of its turn."""
    out = list(chain)
    n = len(chain)
    rng = range(0, n - 1) if closed else range(1, n - 1)
    for i in rng:
        if closed and i == 0:
            continue  # ring start vertex stays fixed
        p = chain[i - 1] if i > 0 else chain[n - 2]
        q, r = chain[i], chain[i + 1]
        l1 = abs(q[0] - p[0]) + abs(q[1] - p[1]); l2 = abs(r[0] - q[0]) + abs(r[1] - q[1])
        d1 = ((q[0] - p[0]) / l1, (q[1] - p[1]) / l1)
        d2 = ((r[0] - q[0]) / l2, (r[1] - q[1]) / l2)
        out[i] = (q[0] + a * (d2[0] - d1[0]), q[1] + a * (d2[1] - d1[1]))
    return out


# ---------------------------------------------------------------- per arc
COLLAPSE = "consec"   # "tie": collapse tie pairs with equal turn sign; "consec": any consecutive same-sign kept pair


def simplify_chain(D, orig, a, closed):
    """DP on the ORIGINAL corners (farthest original corner from the original chord); a span is
    split when the displaced segment deviates more than a from the original chain."""
    n = len(D)
    keep = {0, n - 1}
    ties = set()

    def rec(i, j):
        if j <= i + 1:
            return
        if CHORD == 'orig':
            ds = [dist_line(orig[m], orig[i], orig[j]) for m in range(i + 1, j)]
        elif CHORD == 'disp_orig':
            ds = [dist_line(orig[m], D[i], D[j]) for m in range(i + 1, j)]
        else:
            ds = [dist_line(D[m], D[i], D[j]) for m in range(i + 1, j)]
        md = max(ds)
        degenerate = math.dist(D[i], D[j]) < 1e-12
        if not degenerate:
            if SPLIT == 'dpd':
                if max(dist_line(D[m], D[i], D[j]) for m in range(i + 1, j)) <= TOL:
                    return
            elif SPLIT == 'dpo':
                if max(dist_line(orig[m], D[i], D[j]) for m in range(i + 1, j)) <= TOL:
                    return
            elif SPLIT == 'mid':
                mids = []
                for m in range(i, j):
                    p, q = orig[m], orig[m + 1]
                    L_ = int(round(abs(q[0] - p[0]) + abs(q[1] - p[1])))
                    for u in range(L_):
                        t = (u + 0.5) / L_
                        mids.append((p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t))
                if max(dist_line(mp, D[i], D[j]) for mp in mids) <= (a if TOL < 0 else TOL) + EPS:
                    return
            elif SPLIT == 'midseg':
                mids = []
                for m in range(i, j):
                    p, q = orig[m], orig[m + 1]
                    L_ = int(round(abs(q[0] - p[0]) + abs(q[1] - p[1])))
                    for u in range(L_):
                        t = (u + 0.5) / L_
                        mids.append((p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t))
                O_ = np.array(mids, float)
                if float(_pts_to_poly(O_, np.array([D[i], D[j]], float)).max()) <= TOL:
                    return
            elif SPLIT == 'interior':
                if seg_dev(D[i], D[j], orig[i:j + 1], 1e-6) < a - 1e-9:
                    return
            elif seg_dev(D[i], D[j], orig[i:j + 1]) <= a + EPS:
                return
        idx = [i + 1 + m for m, d in enumerate(ds) if d >= md - 1e-9]
        i1, i2 = idx[0], idx[-1]
        if TIE == 'first':
            i2 = i1
        keep.add(i1)
        if i2 != i1:
            keep.add(i2)
            ties.add((i1, i2))
            rec(i, i1); rec(i1, i2); rec(i2, j)
        else:
            rec(i, i1); rec(i1, j)

    rec(0, n - 1)
    if FORCE is not None: keep = set(FORCE)
    if DEBUG: print("kept", sorted(keep))
    ks = sorted(keep)
    tchain = orig[:-1] if closed else orig
    O = np.asarray(orig, float)

    def pdist(p):
        return float(_pts_to_poly(np.array([p], float), O)[0])

    def kturn(mm):
        p, q, r = D[ks[mm - 1]], D[ks[mm]], D[ks[mm + 1]]
        return (q[0] - p[0]) * (r[1] - q[1]) - (q[1] - p[1]) * (r[0] - q[0])

    out = [D[ks[0]]]
    m = 1
    while m < len(ks) - 1:
        i0 = ks[m]
        if m + 1 < len(ks) - 1:
            i1 = ks[m + 1]
            if kturn(m) * kturn(m + 1) > 0:
                X = inter(out[-1], D[i0], D[i1], D[ks[m + 2]])
                if X is not None and pdist(X) <= a + EPS:
                    out.append(X); m += 2
                    continue
                # clamp: slide from D[i0] along the line prev->D[i0] (towards X) to distance a
                p0 = D[i0]
                dx, dy = p0[0] - out[-1][0], p0[1] - out[-1][1]
                if X is not None:
                    dx, dy = X[0] - p0[0], X[1] - p0[1]
                L = math.hypot(dx, dy)
                ux, uy = dx / L, dy / L
                lo, hi = 0.0, None
                step = 0.002
                t = step
                while t < 1e3:
                    if pdist((p0[0] + ux * t, p0[1] + uy * t)) > a + EPS:
                        hi = t
                        break
                    lo = t
                    t += step
                if hi is None:
                    hi = lo
                for _ in range(80):
                    mid = (lo + hi) / 2
                    if pdist((p0[0] + ux * mid, p0[1] + uy * mid)) <= a:
                        lo = mid
                    else:
                        hi = mid
                out.append((p0[0] + ux * lo, p0[1] + uy * lo)); m += 1
                continue
        out.append(D[i0]); m += 1
    out.append(D[ks[-1]])
    return out


def poly_dev(o, orig):
    return max(seg_dev(p, q, orig) for p, q in zip(o, o[1:]))


FINE = 8


def _cand(chain, closed, a):
    o = simplify_chain(displace(chain, a, closed), chain, a, closed)
    if o is None or len(o) < (4 if closed else 2):
        return None
    d = poly_dev(o, chain)
    if d > a + EPS:
        return None
    return (len(o), d, o)


def simplify_arc(chain, closed=False, kmax=KMAX):
    """chain: corner points (cell units), collinear points removed. Closed: chain[0]==chain[-1]."""
    if len(chain) <= 2:
        return list(chain)
    grid = {}
    for k in range(kmax + 1):
        c = _cand(chain, closed, a_of(k))
        if c:
            grid[k] = c
    exact = (len(chain), 0.0, chain)
    if not grid:
        return chain
    nmin = min(c[0] for c in grid.values())
    if exact[0] <= nmin:
        return chain
    if MODE == 'grid':
        k = min(grid, key=lambda k: (grid[k][0], grid[k][1]))
        return grid[k][2]
    # continuous minimiser of the deviation among candidates with the fewest vertices,
    # then the grid point on its large-a side (ArcGIS's search returns that end)
    step = 0.29 / 32
    best = None
    for k in range(kmax + 1):
        for j in range(FINE):
            if j and k == kmax:
                break
            a = a_of(k) - j * step / FINE
            c = grid.get(k) if j == 0 else _cand(chain, closed, a)
            if c and c[0] == nmin and (best is None or c[1] < best[0] - 1e-12):
                best = (c[1], k, j)
    k = best[1]
    while k >= 0 and not (k in grid and grid[k][0] == nmin):
        k -= 1
    if k < 0:
        k = min(grid, key=lambda k: (grid[k][0], grid[k][1]))
    return grid[k][2]


# ---------------------------------------------------------------- topology
def label_regions(arr, nodata=None):
    H, W = arr.shape
    lab = -np.ones((H, W), int)
    vals = []
    for s in range(H * W):
        i, j = divmod(s, W)
        if lab[i, j] >= 0:
            continue
        v = arr[i, j]
        if nodata is not None and v == nodata:
            continue
        rid = len(vals); vals.append(v); lab[i, j] = rid
        st = [(i, j)]
        while st:
            a, b = st.pop()
            for c, d in ((a, b - 1), (a, b + 1), (a - 1, b), (a + 1, b)):
                if 0 <= c < H and 0 <= d < W and lab[c, d] < 0 and arr[c, d] == v and not (
                        nodata is not None and arr[c, d] == nodata):
                    lab[c, d] = rid; st.append((c, d))
    return lab, vals


def simplify_polygons(raster_array, cellsize, origin, nodata=None, kmax=KMAX):
    arr = np.asarray(raster_array)
    H, W = arr.shape
    lab, vals = label_regions(arr, nodata)

    def L(i, j):  # cell label, row i (from top), col j
        if 0 <= i < H and 0 <= j < W:
            return lab[i, j]
        return -1

    # corners in cell units: x = j, y = H - i. Cells around corner (x, y):
    def around(x, y):
        i = H - y
        return (L(i - 1, x - 1), L(i - 1, x), L(i, x - 1), L(i, x))  # UL UR LL LR

    # directed edges with region on the right (clockwise rings), keyed per region
    edges = {}
    for i in range(H):
        for j in range(W):
            r = lab[i, j]
            if r < 0:
                continue
            x0, y1 = j, H - i  # upper-left corner
            if L(i - 1, j) != r:
                edges.setdefault(r, []).append(((x0, y1), (x0 + 1, y1)))
            if L(i, j + 1) != r:
                edges.setdefault(r, []).append(((x0 + 1, y1), (x0 + 1, y1 - 1)))
            if L(i + 1, j) != r:
                edges.setdefault(r, []).append(((x0 + 1, y1 - 1), (x0, y1 - 1)))
            if L(i, j - 1) != r:
                edges.setdefault(r, []).append(((x0, y1 - 1), (x0, y1)))

    def is_node(c):
        ul, ur, ll, lr = around(*c)
        s = {ul, ur, ll, lr}
        if len(s) >= 3:
            return True
        if len(s) == 2 and ul == lr and ur == ll:
            return True
        return False

    # build rings per region as corner sequences (NO_SIMPLIFY topology)
    simplified_arc = {}

    def arc_key(seq):
        return (seq[0], seq[1], seq[-1])

    def leftval(sub):
        p, q = sub[0], sub[1]
        dx, dy = q[0] - p[0], q[1] - p[1]
        cx = (p[0] + q[0]) / 2 - dy * 0.5
        cy = (p[1] + q[1]) / 2 + dx * 0.5
        lb = L(H - 1 - math.floor(cy), math.floor(cx))
        return math.inf if lb < 0 else vals[lb]

    def forward(r, sub):
        if sub[0] == sub[-1]:
            # closed ring: clockwise from its start
            ar = sum(p[0] * q[1] - q[0] * p[1] for p, q in zip(sub, sub[1:]))
            return ar < 0
        # open arc: starts at the node that is last in raster order (lowest y, then highest x)
        return (sub[0][1], -sub[0][0]) < (sub[-1][1], -sub[-1][0])

    out = []
    for r in range(len(vals)):
        E = edges.get(r, [])
        frm = {}
        for k, e in enumerate(E):
            frm.setdefault(e[0], []).append(k)
        used = [False] * len(E)
        rings = []
        for f in range(len(E)):
            if used[f]:
                continue
            ring = [E[f][0]]; cur = f
            while True:
                used[cur] = True
                a, b = E[cur]; ring.append(b)
                d = (b[0] - a[0], b[1] - a[1])
                cand = [n for n in frm.get(b, []) if not used[n]]
                if not cand:
                    break
                # at a diagonal touch turn right (map coords, region on right)
                def tv(n):
                    p, q = E[n]
                    e = (q[0] - p[0], q[1] - p[1])
                    return -(d[0] * e[1] - d[1] * e[0])
                cur = max(cand, key=tv)
            rings.append(ring)  # closed list of unit-step corners
        poly_rings = []
        for ring in rings:
            pts = ring[:-1]
            n = len(pts)
            nodes = [k for k, c in enumerate(pts) if is_node(c)]
            res = []
            if not nodes:
                # closed arc, start at the corner with smallest y, then largest x
                s = min(range(n), key=lambda k: (pts[k][1], -pts[k][0]))
                seq = pts[s:] + pts[:s] + [pts[s]]
                res = simp_closed(seq, simplified_arc, kmax, forward(r, seq))
            else:
                s = nodes[0]
                seq = pts[s:] + pts[:s] + [pts[s]]
                nidx = [k for k, c in enumerate(seq) if k == 0 or k == len(seq) - 1 or is_node(c)]
                res = [seq[0]]
                for a_, b_ in zip(nidx, nidx[1:]):
                    sub = seq[a_:b_ + 1]
                    res += simp_open(sub, simplified_arc, kmax, forward(r, sub))[1:]
            area = sum(p[0] * q[1] - q[0] * p[1] for p, q in zip(res, res[1:])) / 2
            poly_rings.append((abs(area), res))
        poly_rings.sort(key=lambda t: -t[0])
        ox, oy = origin
        oy_b = oy - H * cellsize
        rings_map = [[(ox + x * cellsize, oy_b + y * cellsize) for x, y in rg] for _, rg in poly_rings]
        out.append({'value': vals[r], 'rings': rings_map})
    return out


def dedup(seq):
    """unit-step corner list -> corners with collinear points removed (keeps ends)."""
    out = [seq[0]]
    for k in range(1, len(seq) - 1):
        p, q, r = out[-1], seq[k], seq[k + 1]
        if (q[0] - p[0]) * (r[1] - q[1]) - (q[1] - p[1]) * (r[0] - q[0]) != 0:
            out.append(q)
    out.append(seq[-1])
    return out


def simp_open(sub, cache, kmax, fwd=True):
    if not fwd:
        return simp_open(sub[::-1], cache, kmax, True)[::-1]
    key = (tuple(sub[0]), tuple(sub[-1]), len(sub), tuple(sub[1]))
    rkey = (tuple(sub[-1]), tuple(sub[0]), len(sub), tuple(sub[-2]))
    if rkey in cache:
        return cache[rkey][::-1]
    if key not in cache:
        cache[key] = simplify_arc(dedup(sub), False, kmax)
    return cache[key]


def simp_closed(seq, cache, kmax, fwd=True):
    if not fwd:
        return simp_closed(seq[::-1], cache, kmax, True)[::-1]
    ch = dedup(seq)
    # start vertex may be collinear (cannot be on a straight run if it is the lowest-right corner)
    key = ('closed', frozenset(seq))
    if key in cache:
        cseq, res = cache[key]
        return res if cseq[1] == seq[1] else res[::-1]
    res = simplify_arc(ch, True, kmax)
    cache[key] = (seq, res)
    return res
