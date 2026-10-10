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
- [x] Field mapping, null handling and value formatting verified against ArcGIS Pro 3.7
      through the ArcGIS bridge; `.dbf` output is byte-identical to ArcGIS's for the same input
- [ ] `.sbn/.sbx` spatial index, `.shp.xml` metadata
- [ ] DOS code pages (437/850/852) in `.dbf`; memo `.dbt` files

## Phase 2 — Geometry engine
Overlay is delegated to `geo` (i_overlay); `geoforge-geometry` adds Esri semantics and
`geoforge-tools` the ArcGIS tools. Each tool is checked against ArcGIS Pro output stored
in `testdata/arcgis-reference/` (regenerate: `scripts/make-arcgis-reference.py`).
- [x] Clip (points, lines, polygons; clip features dissolved; Z/M restored) — matches
      ArcGIS within its 0.001 m XY tolerance
- [x] Buffer, planar: round/flat ends, dissolve NONE/ALL, negative polygon buffers;
      `BUFF_DIST`/`ORIG_FID` — matches ArcGIS shape within 0.2 m, area within 0.01%,
      point buffers exactly (ArcGIS arc densification rule reproduced)
- [ ] Buffer: true arcs in geodatabase output (ArcGIS stores buffers as circular arcs),
      dissolve LIST, line_side LEFT/RIGHT/OUTSIDE_ONLY, geodesic for geographic data
- [ ] Intersect, Union, Erase, Dissolve, Identity, Split, Merge, Append
- [ ] Validation + repair (Check/Repair Geometry), spatial index (R-tree) for overlays
- [ ] Shapefile writer: `.sbn/.sbx` and `.shp.xml` (ArcGIS writes both for tool outputs)

## Phase 3 — File Geodatabase
Format notes: [`docs/formats/filegdb.md`](formats/filegdb.md). Clean-room: no Esri SDK.
- [x] Read: `.gdbtable` / `.gdbtablx` v3 and v4 (64-bit OBJECTIDs), incl. sparse v4 tables
      (page map worked out beyond the public spec; OBJECTIDs exact where GDAL's are not)
- [x] Read: system catalog, feature datasets (`GDB_Items` paths), all field types incl.
      Pro 3.2 BigInteger/DateOnly/TimeOnly/TimestampOffset, high-precision dates, UTF-8/16
- [x] Read: geometry blobs for all types incl. Z/M, MultiPatch (incl. Triangles), true curves
      (circular arc, Bézier, elliptic arc kept as Esri segments)
- [x] Oracle: 137 of 145 test tables match GDAL; `gf describe` / `gf copy-features` from `.gdb`
- [x] Curve densification (`Geometry::densify_curves`); shapefile export densifies like ArcGIS
- [ ] Compressed (`.cdf`) tables
- [x] Domains (214 checked against GDAL), field domains, relationship classes (16 checked);
      subtypes parsed from the documented XML (no sample data to verify against yet)
- [x] Spatial (`.spx`) and attribute (`.atx`) index queries (`features_in_extent`,
      `search_attribute`): 109 + 31 indexes never miss vs. full scans; ~18× faster extent query
- [x] Write new geodatabases: feature classes and tables (all field types except raster),
      all geometry types incl. Z/M, MultiPatch and curves, spatial index (.spx), domains;
      ArcGIS default resolutions (coordinates snap like ArcGIS); high-precision dates only
      when needed. 203 inputs round-trip; GDAL reads every output and its spatial filter
      matches ours on the written index.
- [ ] Write: add to an existing geodatabase (system tables must be rewritten with their indexes)
- [ ] Write: per-vertex missing M values mixed with present ones (encoding unknown)
- [ ] Write: attribute indexes, subtypes, relationship classes, feature datasets
- [x] Written geodatabases open in ArcGIS Pro 3.7: Describe, counts, domains, Check Geometry
      (no problems beyond those already in the source data), Copy Features out of them
- [ ] Projected coordinate systems: use ArcGIS's projection-derived XY domain origin (needs Phase 4)

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
