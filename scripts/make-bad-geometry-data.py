"""Writes shapefiles with deliberately broken geometries to testdata/geoproc/ for the
Check Geometry / Repair Geometry reference cases. Written byte by byte (stdlib only)
because GDAL fixes ring orientation and refuses some of these on write.

    python3 scripts/make-bad-geometry-data.py
"""

import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "testdata" / "geoproc"
PRJ = (DATA / "parcels.prj").read_text()

X0, Y0 = 500000.0, 5000000.0


def sq(x, y, s, cw=True):
    """A closed square ring with its lower-left corner at (x, y)."""
    r = [(x, y), (x, y + s), (x + s, y + s), (x + s, y), (x, y)]
    return r if cw else r[::-1]


def at(ring, dx):
    return [(X0 + dx + x, Y0 + y) for x, y in ring]


# (id, description, parts) — parts are lists of (x, y) relative to (X0 + 100 * id, Y0).
POLYGONS = [
    ("valid square", [sq(0, 0, 50)]),
    ("counter-clockwise outer ring", [sq(0, 0, 50, cw=False)]),
    ("bowtie", [[(0, 0), (0, 50), (50, 0), (50, 50), (0, 0)]]),
    ("unclosed ring", [sq(0, 0, 50)[:-1]]),
    ("null", None),
    ("duplicate vertex", [[(0, 0), (0, 50), (0, 50), (50, 50), (50, 0), (0, 0)]]),
    ("short segment", [[(0, 0), (0, 50), (0.0005, 50), (50, 50), (50, 0), (0, 0)]]),
    ("degenerate ring", [[(0, 0), (0, 50), (0, 0)]]),
    ("overlapping rings", [sq(0, 0, 50), sq(25, 25, 50)]),
    ("clockwise hole", [sq(0, 0, 50), sq(10, 10, 20)]),
    ("hole outside", [sq(0, 0, 40), sq(60, 0, 20, cw=False)]),
    ("spike", [[(0, 0), (0, 50), (25, 50), (25, 80), (25, 50), (50, 50), (50, 0), (0, 0)]]),
    ("zero area", [[(0, 0), (0, 25), (0, 50), (0, 0)]]),
    ("ring touching itself", [[(0, 0), (0, 50), (25, 25), (50, 50), (50, 0), (25, 25), (0, 0)]]),
    ("valid with hole", [sq(0, 0, 50), sq(10, 10, 20, cw=False)]),
    ("near-duplicate vertex", [[(0, 0), (0, 50), (0.002, 50), (50, 50), (50, 0), (0, 0)]]),
    ("notch near own edge", [[(0, 0), (0, 50), (50, 50), (50, 30), (0.0012, 25), (50, 20), (50, 0), (0, 0)]]),
    ("outer in outer", [sq(0, 0, 50), sq(10, 10, 20)]),
    ("hole in hole", [sq(0, 0, 50), sq(5, 5, 40, cw=False), sq(10, 10, 20, cw=False)]),
    ("duplicate hole", [sq(0, 0, 50), sq(10, 10, 20, cw=False), sq(10, 10, 20, cw=False)]),
    ("rings sharing edge", [sq(0, 0, 20), sq(20, 0, 20)]),
]

LINES = [
    ("valid line", [[(0, 0), (50, 50)]]),
    ("duplicate vertex", [[(0, 0), (25, 25), (25, 25), (50, 50)]]),
    ("single point part", [[(0, 0), (50, 50)], [(60, 60)]]),
    ("zero length", [[(10, 10), (10, 10)]]),
    ("null", None),
    ("short segment", [[(0, 0), (25, 25), (25.0004, 25), (50, 50)]]),
    ("self crossing", [[(0, 0), (50, 50), (50, 0), (0, 50)]]),
    ("backtrack", [[(0, 0), (50, 0), (25, 0), (25, 50)]]),
    ("closed loop", [[(0, 0), (0, 50), (50, 50), (0, 0)]]),
    ("T junction", [[(0, 0), (50, 0)], [(25, 0), (25, 50)]]),
    ("end on own middle", [[(0, 50), (0, 0), (30, 0), (30, 20), (0, 20)]]),
    ("overlapping parts", [[(0, 0), (50, 0)], [(25, 0), (75, 0)]]),
    ("near pass", [[(0, 0), (50, 0), (50, 50), (25, 0.0005), (0, 50)]]),
    ("near-duplicate vertex", [[(0, 0), (25, 25), (25.002, 25), (50, 0)]]),
    ("parts touching ends", [[(0, 0), (50, 0)], [(50, 0), (50, 50)]]),
]


def write(name, shape_type, features, out=DATA):
    """features: list of (description, parts or None)."""
    records = []
    for i, (_, parts) in enumerate(features):
        if parts is None:
            records.append(struct.pack("<i", 0))
            continue
        pts = [p for part in parts for p in at(part, 100 * i)]
        xs, ys = [p[0] for p in pts], [p[1] for p in pts]
        bbox = (min(xs), min(ys), max(xs), max(ys))
        starts, n = [], 0
        for part in parts:
            starts.append(n)
            n += len(part)
        rec = struct.pack("<i4d2i", shape_type, *bbox, len(parts), len(pts))
        rec += struct.pack(f"<{len(starts)}i", *starts)
        rec += b"".join(struct.pack("<2d", *p) for p in pts)
        records.append(rec)
    allpts = [p for _, parts in features if parts for part in parts for p in at(part, 100 * features.index((_, parts)))]
    bbox = (min(p[0] for p in allpts), min(p[1] for p in allpts), max(p[0] for p in allpts), max(p[1] for p in allpts))

    def header(length_words):
        return struct.pack(">7i", 9994, 0, 0, 0, 0, 0, length_words) + struct.pack(
            "<2i8d", 1000, shape_type, *bbox, 0, 0, 0, 0
        )

    shp, shx = bytearray(), bytearray()
    offset = 50
    for i, rec in enumerate(records):
        shx += struct.pack(">2i", offset, len(rec) // 2)
        shp += struct.pack(">2i", i + 1, len(rec) // 2) + rec
        offset += 4 + len(rec) // 2
    (out / f"{name}.shp").write_bytes(header(50 + len(shp) // 2) + shp)
    (out / f"{name}.shx").write_bytes(header(50 + len(shx) // 2) + shx)

    # dBASE III with ID (N 10) and DESCR (C 40).
    fields = [(b"ID", b"N", 10, 0), (b"DESCR", b"C", 40, 0)]
    rec_len = 1 + sum(f[2] for f in fields)
    dbf = struct.pack("<B3BIHH20x", 3, 126, 1, 1, len(features), 32 + 32 * len(fields) + 1, rec_len)
    for fname, ftype, flen, fdec in fields:
        dbf += struct.pack("<11sc4xBB14x", fname, ftype, flen, fdec)
    dbf += b"\r"
    for i, (descr, _) in enumerate(features):
        dbf += b" " + str(i + 1).rjust(10).encode() + descr.encode().ljust(40)
    dbf += b"\x1a"
    (out / f"{name}.dbf").write_bytes(dbf)
    (out / f"{name}.prj").write_text(PRJ)
    (out / f"{name}.cpg").write_text("UTF-8")


if __name__ == "__main__":
    write("bad_polygons", 5, POLYGONS)
    write("bad_lines", 3, LINES)
