#!/usr/bin/env python3
"""Verify an extracted CLI alpha using only Python's standard library and ug.

Use --require-clean-runtime inside the minimal runtime image. No GDAL oracle or
build tools are used. This is an installation check, not ArcGIS parity evidence.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import struct
import subprocess
import tempfile


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify_package(package):
    manifest = json.loads((package / "manifest.json").read_text())
    report = json.loads((package / "readiness.json").read_text())
    digest = hashlib.sha256((package / "bin/ug").read_bytes()).hexdigest()
    require(digest == manifest.get("binary_sha256") == report.get("binary_sha256"),
            "Packaged binary does not match manifest and readiness report")
    require(report.get("passed") is True, "Packaged readiness report did not pass")
    require(hashlib.sha256((package / "share/proj/proj.db").read_bytes()).hexdigest()
            == manifest.get("proj_db_sha256"), "Bundled PROJ database does not match manifest")
    return manifest


def check_clean_runtime():
    forbidden = ("ogrinfo", "ogr2ogr", "gdalinfo", "gdal_translate", "proj", "cargo", "rustc", "gcc", "clang")
    found = [name for name in forbidden if shutil.which(name)]
    require(not found, f"Clean runtime contains unexpected tools: {found}")
    require(not Path("/usr/share/proj/proj.db").exists(), "System PROJ database masks packaging defects")


def run(ug, directory, *args):
    env = os.environ.copy()
    for key in ("PROJ_DATA", "PROJ_LIB", "LD_LIBRARY_PATH", "LD_PRELOAD"):
        env.pop(key, None)
    env["PROJ_NETWORK"] = "OFF"
    result = subprocess.run([str(ug), *map(str, args)], cwd=directory, env=env,
                            text=True, capture_output=True, timeout=120)
    require(result.returncode == 0, f"ug {args}: exit {result.returncode}\n{result.stderr}")
    return result.stdout


def read_points(path, expected_crs):
    with sqlite3.connect(f"file:{path}?mode=ro", uri=True) as db:
        require(db.execute("PRAGMA integrity_check").fetchone() == ("ok",), "Invalid GeoPackage")
        table, column, crs = db.execute(
            "SELECT table_name, column_name, srs_id FROM gpkg_geometry_columns").fetchone()
        require(crs == expected_crs, f"Unexpected CRS: {crs}")
        quote = lambda value: '"' + value.replace('"', '""') + '"'
        rows = db.execute(f'SELECT ID, {quote(column)} FROM {quote(table)} ORDER BY ID').fetchall()
    points = []
    for identity, blob in rows:
        require(blob[:2] == b"GP", "Missing GeoPackage geometry header")
        envelope = (blob[3] >> 1) & 7
        require(envelope <= 4, "Invalid geometry envelope")
        offset = 8 + (0, 32, 48, 48, 64)[envelope]
        wkb = blob[offset:]
        endian = "<" if wkb[0] == 1 else ">"
        require(struct.unpack(endian + "I", wkb[1:5])[0] == 1, "Expected 2D Point WKB")
        points.append((identity, *struct.unpack(endian + "dd", wkb[5:21])))
    return points


def exercise(package):
    ug = package / "ug"
    with tempfile.TemporaryDirectory(prefix="unigis runtime with spaces ") as temp:
        directory = Path(temp)
        require("Usage:" in run(ug, directory, "--help"), "Launcher help failed")
        require("buffer" in run(ug, directory, "tools").lower(), "Tool registry unavailable")
        csv = directory / "points.csv"
        csv.write_text("ID,X,Y\n1,12,45\n2,-73,40\n")
        source, projected, restored = (directory / name for name in ("source.gpkg", "projected.gpkg", "restored.gpkg"))
        run(ug, directory, "xy-table-to-point", csv, source / "points", "X", "Y")
        original = read_points(source, 4326)
        require(len(original) == 2, "CSV import changed row count")
        for expected, actual in zip([(1, 12.0, 45.0), (2, -73.0, 40.0)], original):
            require(expected[0] == actual[0] and all(abs(a - b) < 1e-10
                    for a, b in zip(expected[1:], actual[1:])), f"CSV import changed rows: {original}")
        run(ug, directory, "project", source / "points", projected / "points", "3857")
        actual = read_points(projected, 3857)
        require(len(actual) == 2, "Project lost rows")
        for (identity, lon, lat), (result_id, x, y) in zip(original, actual):
            expected = (6378137 * math.radians(lon),
                        6378137 * math.log(math.tan(math.pi / 4 + math.radians(lat) / 2)))
            require(identity == result_id and abs(x - expected[0]) < 0.001
                    and abs(y - expected[1]) < 0.001, "Projected coordinates differ from Web Mercator formula")
        run(ug, directory, "project", projected / "points", restored / "points", "4326")
        returned = read_points(restored, 4326)
        require(len(returned) == len(original), "Inverse project lost rows")
        for first, last in zip(original, returned):
            require(first[0] == last[0] and all(abs(a - b) < 1e-8 for a, b in zip(first[1:], last[1:])),
                    "Projection round trip changed coordinates or attributes")
    return ["launcher", "registry", "csv_to_geopackage", "bundled_proj_forward", "inverse_projection"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--require-clean-runtime", action="store_true")
    args = parser.parse_args()
    report = {"passed": False, "timestamp_utc": datetime.now(timezone.utc).isoformat(),
              "platform": platform.platform(), "clean_runtime_required": args.require_clean_runtime}
    try:
        package = args.package.resolve()
        report["manifest"] = verify_package(package)
        if args.require_clean_runtime:
            check_clean_runtime()
        report["checks"] = exercise(package)
        report["passed"] = True
    except (OSError, ValueError, sqlite3.Error, subprocess.SubprocessError, struct.error) as error:
        report["error"] = str(error)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
