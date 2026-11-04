# UniGIS CLI alpha

This package contains the Linux x86_64 `ug` command-line engine, its validation
report, and an offline workflow test. It does not include the desktop app or web
app, and is not a claim of full ArcGIS Pro compatibility.

## Run

Extract the archive, then run `./ug --help` or `./ug tools`. The CI build targets
Ubuntu 24.04; use a compatible Linux system with glibc, libstdc++ and SQLite 3.
`ldd ./bin/ug` lists the actual shared-library requirements. The `ug` launcher
selects the included PROJ data unless `PROJ_DATA` is already set. Additional datum
grid files are not included. GDAL is **not** required by the engine. Check the
adjacent `.sha256` file before using the archive.

To repeat the packaged workflow checks, install Python 3 and GDAL command-line
tools (`gdal-bin` on Debian/Ubuntu), then run:

```sh
PROJ_DATA="$PWD/share/proj" python3 smoke.py --ug ./bin/ug --report local-readiness.json
```

The checks create temporary synthetic datasets and verify:

- Point clipping, GeoPackage export and Shapefile reopening, including CRS and attributes.
- FileGDB creation, field calculation, row deletion, export and reopening.
- ASCII grid import, projection assignment, raster algebra, NoData, and preservation
  of an existing result after a failed operation.

These are small deterministic workflows, not performance benchmarks or exhaustive
format/parameter coverage. Consult the repository's `docs/KNOWN_GAPS.md` before
depending on an individual tool. Browser and desktop availability differ from CLI
availability.

## Build and package from source

The root `rust-toolchain.toml` pins Rust. Cargo builds use the committed lockfile;
`scripts/fetch-testdata.sh` pins the GDAL test corpus revision. On Linux install
`gdal-bin libsqlite3-dev sqlite3 cmake` plus a C/C++ compiler. Then:

```sh
scripts/fetch-testdata.sh
scripts/check
cargo build --locked --release -j3 -p unigis-cli
python3 scripts/readiness/smoke.py --ug target/release/ug --report /tmp/cli-readiness.json
python3 scripts/readiness/package.py --ug target/release/ug --report /tmp/cli-readiness.json --output dist
```

The packaging script refuses a failed report or a report for another binary. It
includes the source revision and marks a modified checkout as dirty. Archive
metadata is deterministic for identical inputs; this does not promise identical
binaries across different compilers or Linux systems.
It finds PROJ data and its copyright notice in Cargo's bundled PROJ build. For
a system-PROJ build, pass matching `--proj-data` and `--proj-license` paths explicitly.

GitHub Actions **Build tested CLI alpha** performs the same release build after
the full engine gate and uploads the package as an artifact. It does not create a
public GitHub Release or deploy anything.
