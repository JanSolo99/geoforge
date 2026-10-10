# Roadmap

Built in thin vertical slices: each phase ends with something usable from `gf`
and verified against GDAL/OGR output on real data.

## Phase 0 — Foundations
- [x] Cargo workspace, crate layout
- [ ] CI (fmt, clippy, test) on GitHub Actions
- [ ] Test-data corpus (`testdata/`) + GDAL-based oracle tests (compare our reads to `ogrinfo`/`gdalinfo`)

## Phase 1 — Shapefile (complete spec)
- [ ] `.shp/.shx` all 14 shape types incl. Z/M and MultiPatch
- [ ] `.dbf` (dBASE III/IV, field types C/N/F/L/D/M), `.cpg` encodings
- [ ] `.prj` WKT1 passthrough; write support; `gf info` / `gf convert`

## Phase 2 — Geometry engine
- [ ] Evaluate `geo` / `geos` crates vs native implementation per operation
- [ ] Buffer, clip, intersect, union, dissolve, erase, split, merge, append
- [ ] Validation + repair (`MakeValid` semantics), spatial index (R-tree)

## Phase 3 — File Geodatabase
- [ ] Read: `.gdbtable` / `.gdbtablx`, system catalog (`a00000001`), all field types
- [ ] Read: geometry blobs (all shape types + curves), `.spx` spatial index
- [ ] Domains, subtypes, relationship classes (from `GDB_Items` XML)
- [ ] Write: tables + feature classes, then indexes, then domains/subtypes
- Reference: the OpenFileGDB reverse-engineered spec (GDAL) — clean-room, no Esri SDK

## Phase 4 — CRS
- [ ] PROJ binding (`proj` crate / `proj-sys`) for 1000+ CRSs; EPSG lookup; WKT1/WKT2/PROJJSON
- [ ] Datum transforms with grid shifts

## Phase 5 — Raster
- [ ] GeoTIFF/COG read/write (tiling, compression, overviews)
- [ ] Map algebra (local/focal/zonal/global), reclass, resample
- [ ] Hillshade, slope, aspect, curvature, contours, viewshed, hydrology (fill/flow dir/accumulation/watershed)

## Phase 6 — Toolbox framework
- [ ] Tool trait: typed parameters, validation, progress, cancellation, geoprocessing history
- [ ] Port tools by toolbox (Analysis → Data Management → Spatial Analyst → …)
- [ ] Python bindings via PyO3 (`geoforge` module, arcpy-like API)

## Phase 7 — Databases & other formats
- [ ] GeoPackage / SpatiaLite, PostGIS, GeoJSON, KML, GML, DXF, E00, ERDAS IMG, JPEG2000

## Phase 8 — Rendering & UI
- [ ] wgpu-based 2D map renderer (symbology, labeling, raster stretch)
- [ ] Desktop app shell (decision deferred: Qt via cxx-qt vs Tauri vs egui/iced)
- [ ] Layout view + PDF/PNG/TIFF export; 3D scene view

## Phase 9 — Advanced
- [ ] Network analysis, geocoding, spatial statistics, geostatistics, image classification, point clouds (LAS/LAZ)
