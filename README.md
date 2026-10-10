# GeoForge

A native-Linux GIS platform written in Rust, aiming for parity with ArcGIS Pro:
read/write File Geodatabases, Shapefiles and rasters, and run the standard
ArcToolbox-style analysis on them.

Status: **pre-alpha scaffold.** See [`docs/ROADMAP.md`](docs/ROADMAP.md) for the
phased plan and [`docs/SPEC.md`](docs/SPEC.md) for the full target scope.

## Layout

| Crate | Purpose |
|---|---|
| `geoforge-core` | Geometry model, Esri field types, shared errors |
| `geoforge-shapefile` | `.shp/.shx/.dbf/.prj/.cpg` read/write |
| `geoforge-filegdb` | `.gdb` (FileGDB) read/write |
| `geoforge-proj` | CRS + transformations |
| `geoforge-raster` | Raster model, GeoTIFF, map algebra |
| `geoforge-cli` | `gf` command-line tool |

## Build

```sh
cargo build --workspace
cargo test --workspace
```

Requires Rust ≥ 1.85 (install via `rustup`).
