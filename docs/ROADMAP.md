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
- [x] Intersect (polygon/line/point inputs; overlapping features of one input split
      into regions output once per feature, as ArcGIS does), Erase, Union (planar
      overlay, ArcGIS row order), Dissolve (fields, statistics SUM/MEAN/MIN/MAX/RANGE/
      STD/COUNT/FIRST/LAST, multi/single part, line merging) — all match ArcGIS
- [x] Identity (polygon/line/point inputs; Union's cells and order for polygons), Merge
      (fields matched by name, `ADD_SOURCE_INFO` → `MERGE_SRC`) — match ArcGIS
- [x] Near (radius, location, angle; updates the input in place like ArcGIS) and Spatial
      Join (one to one/many, keep all/common, INTERSECT, WITHIN_A_DISTANCE, (COMPLETELY_)
      CONTAINS/WITHIN with Esri's boundary rule, CLOSEST, distance field) — match ArcGIS
      (distances to ~1e-9 m; ArcGIS computes on its own grid)
- [x] Select Layer By Location (all 13 planar overlap types, search distance, invert;
      `gf select-by-location` writes the selection) and Append (TEST/NO_TEST, fields by
      name, other geometry types skipped with WARNING 000594) — match ArcGIS
- [x] Split (one shapefile per text value, named after it; ERROR 003911 for non-text
      fields) and Near with several near feature classes (`NEAR_FC`) — match ArcGIS
- [ ] Split into a geodatabase; Spatial Join field-map merge rules other than First;
      selection types other than NEW_SELECTION (needs layers)
- [x] Line cracking: Intersect, Identity and Split split line results wherever two
      input lines meet (crossings, touches, shared-stretch ends; also lines of the same
      input), Clip and Erase do not; pieces an overlay split at a mere touch are rejoined.
      Part counts now match ArcGIS in every reference case (checked by the test)
- [ ] Order of parts within cracked lines (ArcGIS's looks engine-internal: sometimes
      reversed or rotated; shapes and part counts match)
- [x] Line×line Intersect (shared stretches), `output_type` POINT/LINE (outline crossings
      per feature pair, incl. ArcGIS's late rows for points inside shared stretches;
      shared polygon boundaries), `join_attributes` ALL/NO_FID/ONLY_FID — match ArcGIS
- [x] Union `join_attributes` and `gaps` (NO_GAPS adds enclosed uncovered areas last, FID
      -1); Identity `join_attributes` (applies to the identity side; NO_FID also drops the
      input FID) and `relationship` (LEFT_/RIGHT_ pairs by side of each line piece; missing
      side 0/"" or -1 with ONLY_FID; points get the columns, all 0). Identity of lines and
      points splits overlapping identity polygons into cells; line rows come in passes
      (uncovered, outer edges, inside/shared edges, overlaps) — match ArcGIS
- [ ] Line Identity/Intersect where input lines overlap each other (ArcGIS writes the shared
      stretch once per line in a late pass, in the first line's direction)
- [x] Overlay Z/M: Clip, Erase, Intersect (incl. POINT/LINE), Union, Identity and Split
      carry Z and M (output Z/M-aware if any input is): each vertex takes them from the
      first input that has them and whose outline passes there (POINT output: polygon
      input first), interpolated along the segment, else Z 0 / no M; rounded to ArcGIS's
      0.0001 Z/M grid (not Split). M-aware lines in Clip get a vertex at every crossing
      with another input line, and the last feature is written first — match ArcGIS
      (within one grid step: ArcGIS rounds as it splits)
- [ ] Z of POINT output where a line crosses a hole at its corners (one case: ArcGIS
      interpolates between the outer-ring crossings; on a plain square it does not)
- [x] Intersect POINT/LINE output of three or more inputs (where every input's outline
      meets; shared stretches cracked at all outline nodes), rows of lower-dimension
      inputs first and line pieces in overlaps in the late pass, repeated field names
      NAME_1, NAME_12, NAME_12_13, ... — match ArcGIS (its cascade moves vertices up
      to ~2 × tolerance)
- [ ] `cluster_tolerance`
- [x] Spatial index: R-tree over envelopes (`EnvelopeIndex`) in Near, Spatial Join,
      Select By Location, Identity and Intersect; nearest search stops once no envelope
      can be closer. 10k parcels × 20k wells: Spatial Join 45 s → 0.06 s, Near 1.9 s → 0.09 s
- [x] Check Geometry and Repair Geometry (ESRI and OGC rules; one problem per feature
      in ArcGIS's order; tolerance cells: segments ≤ 2√2·tol, vertex-to-segment ≤ √2·tol;
      even-odd rebuild of polygons, OGC lines split where they meet; DELETE/KEEP_NULL;
      `.dbf` table output) — match ArcGIS on 34 broken shapes
- [ ] Check/Repair Geometry: geodatabase output tables, curves, multipoint and Z/M
      problems ("empty Z values", "bad envelope", ...)
- [x] Clip/Erase against many polygons: each feature is clipped by the union of only the
      clip polygons whose envelopes reach it (unions cached per candidate set). 10k parcels
      by 10k parcels: 34 s → 0.05 s; by 300 road buffers: 30 s → 1.6 s; 20k wells: 1.6 s → 0.05 s
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
