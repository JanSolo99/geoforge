#!/usr/bin/env python3
"""Snapshot of a file geodatabase's catalog as JSON (read through GDAL, the oracle), for comparing
what ArcGIS and GeoForge leave behind after Delete / Rename (crates/geoforge-filegdb/tests/catalog_ops.rs).

    gdb-snapshot.py X.gdb > X.json

The reference states in testdata/gdb-reference/catalog_ops were made in ArcGIS Pro: before.gdb from
CreateFileGDB, CreateFeatureDataset ds, CreateFeatureclass ds/inds and loose, CreateTable tbl, a coded
domain `dom` assigned to tbl.k, and the relationship class rc (tbl -> loose); each <op>.json is the
snapshot after one of Delete(tbl) delTbl, Delete(ds/inds) delInds, Rename(loose, loose2) renLoose,
Rename(tbl, tbl2) renTbl, Delete(rc) delRc, Rename(rc, rc2) renRc, AlterDomain(dom, dom2) renDom,
Delete(ds) delDs, Rename(ds, ds2) renDs.
"""
import csv, io, json, os, re, subprocess, sys

def table(gdb, name):
    out = subprocess.run(["ogr2ogr", "-f", "CSV", "/vsistdout/", gdb, name, "-oo", "LIST_ALL_TABLES=YES",
                          "-nlt", "NONE", "-lco", "GEOMETRY=AS_WKT"], capture_output=True, text=True, check=True).stdout
    return list(csv.DictReader(io.StringIO(out)))

gdb = sys.argv[1]
items = table(gdb, "GDB_Items")
names = {i["UUID"]: (i["Path"] or i["Name"] or "root") for i in items}
def defaults(gdb):
    out = subprocess.run(["ogrinfo", "-json", "-so", "-al", gdb, "-oo", "LIST_ALL_TABLES=YES"], capture_output=True, text=True).stdout
    try:
        layers = json.loads(out)["layers"]
    except (ValueError, KeyError):
        return {}
    return {l["name"]: [[f["name"], f.get("defaultValue")] for f in l.get("fields", [])] for l in layers if not l["name"].startswith("GDB_")}

snap = {
    "defaults": defaults(gdb),
    "items": sorted(
        ({"name": i["Name"], "physical": i["PhysicalName"], "path": i["Path"], "type": i["Type"][1:9],
          "definition": re.sub(r"\s+", "", i["Definition"] or "")} for i in items),
        key=lambda i: (i["path"], i["name"], i["type"])),
    "rels": sorted(f'{names.get(r["OriginID"], "?")} -> {names.get(r["DestID"], "?")} : {r["Type"][1:9]}'
                   for r in table(gdb, "GDB_ItemRelationships")),
    "catalog": sorted(r["Name"] for r in table(gdb, "GDB_SystemCatalog") if not r["Name"].startswith("GDB_")),
    "tables": len({f[:9] for f in os.listdir(gdb) if re.match(r"a[0-9a-f]{8}\.gdbtable$", f)}),
}
json.dump(snap, sys.stdout, indent=1)
