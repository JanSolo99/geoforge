"""Writes point shapefiles whose .dbf text is in a DOS code page, tagged only by
the language driver ID (no .cpg), to testdata/shp/codepages/.

    python3 scripts/make-codepage-data.py
"""

import struct
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "testdata" / "shp" / "codepages"
CASES = {
    # name: (LDID, code page, values)
    "cp437": (0x01, "cp437", ["Zürich", "Ångström", "naïve café", "½ ± ° ß"]),
    "cp850": (0x02, "cp850", ["Ñandú", "Øresund", "São Paulo", "Ýmir þór"]),
    "cp852": (0x64, "cp852", ["Łódź", "Kraków", "Brno - Šumava", "Győr"]),
    "cp865": (0x66, "cp865", ["Ålesund", "Tromsø", "Æbeltoft", "¤ ø"]),
    "cp866": (0x65, "cp866", ["Москва", "Київ", "Минск", "№ 5"]),
}


def write(name, ldid, codec, values):
    n = len(values)
    # .shp/.shx: points at (i, i).
    recs = [struct.pack("<i2d", 1, float(i), float(i)) for i in range(n)]
    def header(words):
        return struct.pack(">7i", 9994, 0, 0, 0, 0, 0, words) + struct.pack(
            "<2i8d", 1000, 1, 0.0, 0.0, float(n - 1), float(n - 1), 0, 0, 0, 0)
    shp, shx, off = bytearray(), bytearray(), 50
    for i, r in enumerate(recs):
        shx += struct.pack(">2i", off, len(r) // 2)
        shp += struct.pack(">2i", i + 1, len(r) // 2) + r
        off += 4 + len(r) // 2
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"{name}.shp").write_bytes(header(50 + len(shp) // 2) + shp)
    (OUT / f"{name}.shx").write_bytes(header(50 + len(shx) // 2) + shx)
    fields = [(b"ID", b"N", 4, 0), (b"NAME", b"C", 30, 0)]
    rec_len = 1 + sum(f[2] for f in fields)
    dbf = struct.pack("<B3BIHH17xB2x", 3, 126, 1, 1, n, 32 + 32 * len(fields) + 1, rec_len, ldid)
    for fname, ftype, flen, fdec in fields:
        dbf += struct.pack("<11sc4xBB14x", fname, ftype, flen, fdec)
    dbf += b"\r"
    for i, v in enumerate(values):
        dbf += b" " + str(i + 1).rjust(4).encode() + v.encode(codec).ljust(30)
    dbf += b"\x1a"
    (OUT / f"{name}.dbf").write_bytes(dbf)


for name, (ldid, codec, values) in CASES.items():
    write(name, ldid, codec, values)
