# Roadmap

Built in thin vertical slices: each phase ends with something usable from `gf`
and verified against GDAL/OGR output on real data.

## Phase 0 — Foundations
- [x] Cargo workspace, crate layout
- [x] CI (fmt, clippy, test) on GitHub Actions
- [x] Test-data corpus: `testdata/shp` (committed, `scripts/make-testdata.sh`) + `testdata/large` (`scripts/fetch-testdata.sh`)
- [x] `geoforge-testkit`: oracle tests comparing our reads with `ogrinfo` (JSON attributes + ISO WKT geometry)

## Phase 1 — Shapefile (complete spec)
- [x] Esri-layout geometry model in `geoforge-core` (flat coords + part offsets, Z/M, MultiPatch part types)
- [x] `.shp/.shx` all 14 shape types incl. Z/M and MultiPatch; empty points; missing/corrupt `.shx` recovery
- [x] `.dbf` (dBASE III/IV, field types C/N/F/L/D/M), `.cpg` / LDID encodings
- [x] `.prj` WKT1 passthrough
- [x] Writer: ArcGIS field-name rules, null substitution, clockwise shells, 2 GB limit
- [x] `gf describe` (arcpy.Describe) and `gf copy-features` (Copy Features)
- [ ] Verify N/F width → Short/Long/Double mapping and writer widths against ArcGIS Pro output
- [ ] `.sbn/.sbx` spatial index, `.shp.xml` metadata
- [ ] DOS code pages (437/850/852) in `.dbf`; memo `.dbt` files

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
