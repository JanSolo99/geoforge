"""Writes SQLite test databases in testdata/gpkg/ with Python's sqlite3, plus a
JSON dump of every table (the oracle for GeoForge's own SQLite reader).

    python3 scripts/make-sqlite-data.py
"""

import base64
import json
import random
import sqlite3
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "testdata" / "gpkg"
OUT.mkdir(parents=True, exist_ok=True)


def build(name, page_size):
    path = OUT / name
    path.unlink(missing_ok=True)
    con = sqlite3.connect(path)
    con.execute(f"PRAGMA page_size = {page_size}")
    con.execute('CREATE TABLE "mixed" (id INTEGER PRIMARY KEY, i INTEGER, r REAL, t TEXT(30), b BLOB, n)')
    rng = random.Random(42)
    rows = []
    for k in range(1, 801):
        i = rng.choice([0, 1, -1, 127, -128, 32767, -32768, 2**23 - 1, -(2**31), 2**40 + 7, -(2**47), 2**62, None])
        r = rng.choice([0.0, -1.5, 1e300, rng.uniform(-1e6, 1e6), None])
        size = rng.choice([50, 600, 5000, 70000]) if k % 97 == 0 else rng.choice([0, 5, 20])
        t = "".join(rng.choice("abcdéλ漢") for _ in range(size)) if k % 5 else None
        b = bytes(rng.getrandbits(8) for _ in range(rng.choice([0, 1, 100, 3000] if k % 9 == 0 else [0, 1, 10]))) if k % 3 == 0 else None
        rows.append((k * 3, i, r, t, b, rng.choice([None, 4, 2.5, "x"])))
    con.executemany("INSERT INTO mixed VALUES (?, ?, ?, ?, ?, ?)", rows)
    # Rows deleted and others added later, so pages are not in key order.
    con.execute("DELETE FROM mixed WHERE id % 11 = 0")
    con.execute("CREATE TABLE later (a TEXT NOT NULL, b INTEGER)")
    con.executemany("INSERT INTO later VALUES (?, ?)", [(f"row{j}", j) for j in range(500)])
    con.execute("ALTER TABLE later ADD COLUMN c REAL DEFAULT 1.5")
    con.execute("INSERT INTO later VALUES ('new', 1, 2.5)")
    con.commit()
    dump = {}
    for (table,) in con.execute("SELECT name FROM sqlite_master WHERE type = 'table'"):
        out = []
        for row in con.execute(f'SELECT rowid, * FROM "{table}" ORDER BY rowid'):
            out.append([enc(v) for v in row])
        dump[table] = out
    con.close()
    return dump


def enc(v):
    if isinstance(v, bytes):
        return {"blob": base64.b64encode(v).decode()}
    if isinstance(v, float):
        return {"real": v}
    return v


# The same content at three page sizes; one dump for all.
dumps = [build(name, size) for name, size in [("sqlite_4k.sqlite", 4096), ("sqlite_512.sqlite", 512), ("sqlite_64k.sqlite", 65536)]]
assert dumps[0] == dumps[1] == dumps[2]
(OUT / "sqlite_dump.json").write_text(json.dumps(dumps[0]))
