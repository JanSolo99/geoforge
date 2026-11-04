#!/usr/bin/env python3
"""Test the actual ug binary through three complete, offline file workflows.

GDAL is a test oracle only. All inputs are synthetic and temporary. Missing
dependencies and timeouts fail the run; they never count as passing coverage.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
from pathlib import Path
import platform
import shutil
import sqlite3
import subprocess
import tempfile
import time


def run(*args, success=True):
    result = subprocess.run([str(x) for x in args], capture_output=True, text=True, timeout=120)
    if (result.returncode == 0) != success:
        raise RuntimeError(f"{args}: exit {result.returncode}\n{result.stdout}\n{result.stderr}")
    return result.stdout


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def layer(path, name=None):
    data = json.loads(run("ogrinfo", "-json", "-features", "-nomd", path, *([name] if name else [])))
    require(len(data["layers"]) == 1, f"Expected one layer: {path}")
    return data["layers"][0]


def points(directory):
    source = directory / "points.geojson"
    source.write_text(json.dumps({"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {"ID": i},
         "geometry": {"type": "Point", "coordinates": [500000 + i * 10, 5000000]}}
        for i in (1, 2, 3)
    ]}))
    shp = directory / "points.shp"
    run("ogr2ogr", "-f", "ESRI Shapefile", "-a_srs", "EPSG:32633", shp, source)
    return shp


def assert_points(data, ids):
    require(data["featureCount"] == len(ids), "Feature count changed")
    features = sorted(data["features"], key=lambda f: f["properties"]["ID"])
    require([f["properties"]["ID"] for f in features] == ids, "Attributes changed")
    for feature, i in zip(features, ids):
        xy = feature["geometry"]["coordinates"][:2]
        require(all(abs(a - b) < 0.001 for a, b in zip(xy, [500000 + i * 10, 5000000])),
                "Coordinates changed")
    wkt = data["geometryFields"][0]["coordinateSystem"]["wkt"]
    require('ID["EPSG",32633]' in wkt or 'AUTHORITY["EPSG","32633"]' in wkt,
            "EPSG:32633 was not preserved")


def vector(ug, directory):
    source = points(directory)
    mask = directory / "mask.geojson"
    mask.write_text(json.dumps({"type": "FeatureCollection", "features": [
        {"type": "Feature", "properties": {}, "geometry": {"type": "Polygon", "coordinates": [[
            [500005, 4999995], [500025, 4999995], [500025, 5000005],
            [500005, 5000005], [500005, 4999995]
        ]]}}
    ]}))
    run("ogr2ogr", "-a_srs", "EPSG:32633", directory / "mask.shp", mask)
    gpkg = directory / "clipped.gpkg"
    run(ug, "clip", source, directory / "mask.shp", gpkg / "clipped")
    assert_points(layer(gpkg, "clipped"), [1, 2])
    reopened = directory / "reopened.shp"
    run(ug, "copy-features", gpkg / "clipped", reopened)
    assert_points(layer(reopened), [1, 2])
    with sqlite3.connect(f"file:{gpkg}?mode=ro", uri=True) as db:
        require(db.execute("PRAGMA integrity_check").fetchall() == [("ok",)],
                "GeoPackage integrity check failed")


def geodatabase(ug, directory):
    source = points(directory)
    gdb = directory / "edited.gdb"
    table = gdb / "points"
    run(ug, "copy-features", source, table)
    run(ug, "add-field", table, "SCORE", "LONG")
    run(ug, "calculate-field", table, "SCORE", "!ID! * 2")
    run(ug, "delete-rows", table, "--where", "ID = 3")
    data = layer(gdb, "points")
    assert_points(data, [1, 2])
    require(sorted(f["properties"]["SCORE"] for f in data["features"]) == [2, 4], "Calculated values differ")
    gpkg = directory / "export.gpkg"
    run(ug, "copy-features", table, gpkg / "points")
    exported = layer(gpkg, "points")
    assert_points(exported, [1, 2])
    require(sorted(f["properties"]["SCORE"] for f in exported["features"]) == [2, 4], "Export lost edits")


def raster(ug, directory):
    source = directory / "input.asc"
    source.write_text("ncols 3\nnrows 2\nxllcorner 500000\nyllcorner 5000000\ncellsize 10\nNODATA_value -9999\n1 2 -9999\n4 5 6\n")
    tif = directory / "input.tif"
    run(ug, "run", "ascii_to_raster", f"input={source}", f"output={tif}", "data_type=FLOAT")
    run(ug, "run", "define_projection", f"input={tif}", f"output={tif}", "coordinate_system=EPSG:32633")
    output = directory / "plus.tif"
    run(ug, "plus", tif, "10", output)
    info = json.loads(run("gdalinfo", "-json", output))
    require(info["size"] == [3, 2], "Raster dimensions changed")
    require(info["geoTransform"] == [500000, 10, 0, 5000020, 0, -10], "Raster grid changed")
    require('ID["EPSG",32633]' in info["coordinateSystem"]["wkt"], "Raster CRS lost")
    require(len(info["bands"]) == 1, "Unexpected band count")
    nodata = info["bands"][0]["noDataValue"]
    cells = [float(row.split()[2]) for row in run("gdal_translate", "-q", "-of", "XYZ", output, "/vsistdout/").splitlines()]
    require(cells[:2] + cells[3:] == [11, 12, 14, 15, 16]
            and math.isclose(cells[2], nodata, rel_tol=1e-7),
            f"Raster values/NoData changed: {cells}")
    # A failed rerun must not destroy an existing result.
    before = output.read_bytes()
    run(ug, "plus", directory / "missing.tif", "10", output, success=False)
    require(output.read_bytes() == before, "Failed operation damaged existing output")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ug", type=Path, required=True)
    parser.add_argument("--report", type=Path, default=Path("readiness.json"))
    args = parser.parse_args()
    ug = args.ug.resolve()
    report = {"timestamp_utc": datetime.now(timezone.utc).isoformat(),
              "platform": platform.platform(), "binary": str(ug), "checks": [],
              "scope": "CLI workflows; GDAL format oracle; not desktop/browser or full ArcGIS parity"}
    try:
        for tool in ("ogrinfo", "ogr2ogr", "gdalinfo", "gdal_translate"):
            require(shutil.which(tool), f"Required test oracle missing: {tool}")
        require(ug.is_file(), f"Binary missing: {ug}")
        report["binary_sha256"] = hashlib.sha256(ug.read_bytes()).hexdigest()
        report["gdal"] = run("ogrinfo", "--version").strip()
        for name, check in (("vector_clip_export_reopen", vector), ("filegdb_edit_export_reopen", geodatabase),
                            ("raster_algebra_nodata_failure_safety", raster)):
            start = time.monotonic()
            entry = {"name": name}
            try:
                with tempfile.TemporaryDirectory(prefix="unigis-readiness-") as tmp:
                    check(ug, Path(tmp))
                entry["status"] = "passed"
            except Exception as error:
                entry.update(status="failed", error=str(error))
            entry["seconds"] = round(time.monotonic() - start, 3)
            report["checks"].append(entry)
            print(f"{entry['status']}: {name}", flush=True)
    except Exception as error:
        report["error"] = str(error)
    report["passed"] = len(report["checks"]) == 3 and all(c["status"] == "passed" for c in report["checks"])
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
