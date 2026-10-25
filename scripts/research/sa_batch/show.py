"""Schema, feature count and first rows of reference outputs: show.py name... [-n ROWS]"""
import csv
import io
import json
import subprocess
import sys

D = "testdata/arcgis-reference/"
args = sys.argv[1:]
rows = 3
if "-n" in args:
    i = args.index("-n")
    rows = int(args[i + 1])
    del args[i : i + 2]
for n in args:
    for ext in (".shp", ".dbf"):
        path = D + n + ext
        info = subprocess.run(["ogrinfo", "-json", "-so", "-al", path], capture_output=True, text=True).stdout
        try:
            layer = json.loads(info)["layers"][0]
        except (ValueError, KeyError, IndexError):
            continue
        fields = [(f["name"], f["type"], f.get("width"), f.get("precision")) for f in layer["fields"]]
        geom = layer.get("geometryFields", [{}])[0].get("type") if layer.get("geometryFields") else None
        print(f"== {n}{ext}: {layer['featureCount']} features, {geom}")
        print("   ", fields)
        out = subprocess.run(["ogr2ogr", "-f", "CSV", "/vsistdout/", path, "-lco", "GEOMETRY=AS_WKT"], capture_output=True, text=True).stdout
        for r in list(csv.reader(io.StringIO(out)))[1 : rows + 1]:
            print("   ", [c[:60] for c in r])
        break
