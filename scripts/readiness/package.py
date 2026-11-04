#!/usr/bin/env python3
"""Package only a binary whose hash matches a successful readiness report.

Produces a deterministic archive for the same inputs, plus its SHA-256 digest.
This does not publish a release or change a deployment.
"""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--ug", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--proj-data", type=Path, help="PROJ data directory (otherwise find the bundled Cargo build)")
    parser.add_argument("--proj-license", type=Path, help="PROJ copyright notice (otherwise find it in the Cargo build)")
    args = parser.parse_args()
    report = json.loads(args.report.read_text())
    binary = args.ug.read_bytes()
    digest = hashlib.sha256(binary).hexdigest()
    if report.get("passed") is not True or report.get("binary_sha256") != digest:
        parser.error("refusing to package: report must pass and match the exact binary")
    proj_data = args.proj_data
    if proj_data is None:
        candidates = list(args.ug.resolve().parent.glob("build/proj-sys-*/out/share/proj"))
        if len(candidates) != 1:
            parser.error("provide --proj-data and --proj-license for the PROJ used by this build")
        proj_data = candidates[0]
    if not (proj_data / "proj.db").is_file():
        parser.error("PROJ data directory must contain proj.db")
    license_path = args.proj_license
    if license_path is None:
        notices = list(proj_data.parent.parent.glob("PROJSRC/proj/*/COPYING"))
        if len(notices) != 1:
            parser.error("provide --proj-license for the bundled PROJ data")
        license_path = notices[0]
    root = Path(__file__).resolve().parents[2]
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain", "--untracked-files=normal"], cwd=root, text=True).strip())
    label = f"unigis-cli-alpha-{revision[:12]}" + ("-dirty" if dirty else "")
    manifest = {"source_revision": revision, "working_tree_dirty": dirty, "binary_sha256": digest,
                "proj_db_sha256": hashlib.sha256((proj_data / "proj.db").read_bytes()).hexdigest(),
                "scope": "Linux x86_64 CLI alpha; desktop and browser are not included"}
    launcher = b'''#!/bin/sh
set -eu
base=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
: "${PROJ_DATA:=$base/share/proj}"
export PROJ_DATA
exec "$base/bin/ug" "$@"
'''
    entries = {
        "ug": (launcher, 0o755),
        "bin/ug": (binary, 0o755),
        "PROJ-COPYING": (license_path.read_bytes(), 0o644),
        "readiness.json": (args.report.read_bytes(), 0o644),
        "manifest.json": ((json.dumps(manifest, indent=2) + "\n").encode(), 0o644),
        "README.md": ((root / "docs/ALPHA.md").read_bytes(), 0o644),
        "smoke.py": ((root / "scripts/readiness/smoke.py").read_bytes(), 0o755),
    }
    for file in sorted(proj_data.iterdir()):
        if file.is_file():
            entries[f"share/proj/{file.name}"] = (file.read_bytes(), 0o644)
    args.output.mkdir(parents=True, exist_ok=True)
    archive = args.output / f"{label}.tar.gz"
    with archive.open("wb") as raw, gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
        with tarfile.open(fileobj=compressed, mode="w") as tar:
            for name, (data, mode) in sorted(entries.items()):
                info = tarfile.TarInfo(f"{label}/{name}")
                info.size, info.mode, info.mtime = len(data), mode, 0
                tar.addfile(info, io.BytesIO(data))
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(archive.suffix + ".sha256").write_text(f"{checksum}  {archive.name}\n")
    print(archive)


if __name__ == "__main__":
    main()
