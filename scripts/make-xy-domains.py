#!/usr/bin/env python3
"""Collects the XY domain ArcGIS Pro gives each coordinate system by default, for
crates/geoforge-filegdb/src/xy_domains.tsv.gz.

ArcGIS derives the origin from its own projection engine's idea of where each system is defined (web
Mercator to about 89 degrees, sinusoidal to 89.84, ...), which cannot be reproduced from a formula; this
asks ArcGIS for every projected and geographic system PROJ's database knows (EPSG and ESRI codes) and
keeps what it answers, by the system's Esri name (the name in the `.prj`'s PROJCS/GEOGCS).

    uv run -q --with "mcp>=2" python scripts/make-xy-domains.py

Needs the ArcGIS bridge (tools/arcgis-bridge) and PROJ's proj.db.
"""
import gzip, os, sqlite3, subprocess, sys, tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
db = next(p for p in ("/usr/share/proj/proj.db", "/usr/local/share/proj/proj.db") if os.path.exists(p))
c = sqlite3.connect(db)
codes = [("P", a, k) for a, k in c.execute("select auth_name, code from projected_crs where deprecated=0 and auth_name in ('EPSG','ESRI')")]
codes += [("G", a, k) for a, k in c.execute("select auth_name, code from geodetic_crs where deprecated=0 and auth_name in ('EPSG','ESRI') and type='geographic 2D'")]

work = Path(tempfile.mkdtemp(prefix="xydomains-"))
(work / "codes.txt").write_text("\n".join(" ".join(x) for x in codes) + "\n")
code = f"""
out = []
for ln in open(P({str(work / 'codes.txt')!r})).read().split('\\n'):
    if not ln.strip():
        continue
    kind, auth, code = ln.split()
    try:
        sr = arcpy.SpatialReference(int(code))
        dom = sr.domain.split()
        if len(dom) < 4:
            continue
        out.append('\\t'.join([sr.name, dom[0], dom[1], repr(sr.XYResolution), repr(sr.XYTolerance)]))
    except Exception:
        pass
open(P({str(work / 'domains.tsv')!r}), 'w').write('\\n'.join(out) + '\\n')
print(len(out))
"""
(work / "collect.py").write_text(code)
subprocess.run(["uv", "run", "-q", "--with", "mcp>=2", "python", str(root / "tools/arcgis-bridge/run_code.py"), str(work / "collect.py")], check=True)

by_name = {}
for ln in (work / "domains.tsv").read_text().splitlines():
    name, xo, yo, res, tol = ln.split("\t")
    by_name.setdefault(name, (xo, yo, res, tol))

def short(x):
    return x[:-2] if x.endswith(".0") else x

lines = [
    "# XY domain ArcGIS Pro 3.7 gives each coordinate system by default (scripts/make-xy-domains.py).",
    "# name<TAB>x origin<TAB>y origin<TAB>resolution<TAB>tolerance; resolution and tolerance are empty for 0.0001 / 0.001.",
]
for name, (xo, yo, res, tol) in sorted(by_name.items()):
    lines.append("\t".join([name, short(xo), short(yo), "" if res == "0.0001" else res, "" if tol == "0.001" else tol]))
out = root / "crates/geoforge-filegdb/src/xy_domains.tsv.gz"
with gzip.GzipFile(out, "wb", mtime=0) as f:
    f.write(("\n".join(lines) + "\n").encode())
print(f"{len(by_name)} systems -> {out} ({out.stat().st_size} bytes)")
