"""Index-related files of the user tables in the gdbs of an ops set: file names with sizes and
the .gdbindexes text. Usage: python3 index_files.py DIR_OF_GDBS [op ...]"""
import glob
import hashlib
import os
import re
import sys

root = sys.argv[1]
ops = sys.argv[2:]
for gdb in sorted(glob.glob(root + "/*.gdb")):
    op = os.path.basename(gdb)[:-11]
    if ops and op not in ops:
        continue
    print("==", os.path.basename(gdb))
    tables = sorted(f for f in os.listdir(gdb) if re.fullmatch(r"a0000000[b-f]\.gdbtable", f))
    for t in tables:
        stem = t[:9]
        names = sorted(f for f in os.listdir(gdb) if f.startswith(stem + ".") and not f.endswith(".lock"))
        print(" ", stem, [(n[10:], os.path.getsize(os.path.join(gdb, n))) for n in names if n.endswith((".atx", ".spx", ".gdbindexes"))])
        gi = os.path.join(gdb, stem + ".gdbindexes")
        if os.path.exists(gi):
            data = open(gi, "rb").read()
            text = data.decode("utf-16le", errors="replace")
            print("    gdbindexes", len(data), re.sub(r"[^\x20-\x7e]+", "|", text)[:300])
