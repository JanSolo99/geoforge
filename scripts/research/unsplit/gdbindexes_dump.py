"""Decodes the .gdbindexes of a table: python3 gdbindexes_dump.py FILE [FILE ...]"""
import struct
import sys


def decode(path):
    data = open(path, "rb").read()
    (count,) = struct.unpack_from("<I", data, 0)
    pos = 4
    out = []
    for _ in range(count):
        (n,) = struct.unpack_from("<I", data, pos)
        pos += 4
        name = data[pos : pos + 2 * n].decode("utf-16le")
        pos += 2 * n
        tail = data[pos : pos + 2]  # 00 00
        pos += 2
        m2, m3, m4 = struct.unpack_from("<ihi", data, pos)
        pos += 10
        (n2,) = struct.unpack_from("<I", data, pos)
        pos += 4
        expr = data[pos : pos + 2 * n2].decode("utf-16le")
        pos += 2 * n2 + 2
        out.append((name, tail.hex(), m2, m3, m4, expr))
    return out, pos == len(data)


for f in sys.argv[1:]:
    print(f, decode(f))
