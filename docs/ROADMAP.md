# Roadmap

The verified inventory of ArcGIS Pro's tools, formats and arcpy API (from the official documentation,
2,157 tools) is in [`arcgis-inventory/`](arcgis-inventory/README.md); its `hard_items.md` collects the
documented algorithms of the hard-to-replicate tools.

See [`GAP_ANALYSIS.md`](GAP_ANALYSIS.md) (and `gap_list.csv`) for everything ArcGIS Pro has that
GeoForge does not yet, with difficulty, verifiability against ArcGIS, a recommended order and an
ENVI-parity outlook.

**Policy.** The goal is a native, open-source Linux replica of ArcGIS Pro with all of its features.
Nothing is written off as impossible or out of scope until it has been investigated in depth
(black-box probes through the ArcGIS bridge, the official documentation, public format
specifications); "not yet understood" is the strongest label an item may carry. The three
structural blockers come first, because most of the rest depends on them:
1. **Editing an existing file geodatabase** (rows, fields, indexes, subtypes, relationship
   classes, feature datasets, topology, attribute rules).
2. **The tool framework** (typed parameters, environments, layers and selections, messages,
   geoprocessing history, Python bindings, toolboxes).
3. **The raster stack** (cell size / extent / mask / snap environments, Project Raster, Mosaic,
   Clip, multi-band, pyramids and statistics, more raster formats).

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
- [x] `.sbn/.sbx` spatial index: written with every shapefile and by `gf add-spatial-index`,
      byte-identical to ArcGIS's on all reference outputs (format in docs/formats)
- [ ] `.shp.xml` metadata
- [x] DOS and Mac code pages in `.dbf` by language driver ID (437, 737, 850, 852, 857,
      860, 861, 863, 865, 866, 10000, 10007, 10029; GDAL's LDID table, as ArcGIS reads
      them); memo fields read as GDAL does (block number) — ArcGIS cannot open such files

## Phase 2 — Geometry engine
Overlay is delegated to `geo` (i_overlay); `geoforge-geometry` adds Esri semantics and
`geoforge-tools` the ArcGIS tools. Each tool is checked against ArcGIS Pro output stored
in `testdata/arcgis-reference/` (regenerate: `scripts/make-arcgis-reference.py`).
- [x] Clip (points, lines, polygons; clip features dissolved; Z/M restored) — matches
      ArcGIS within its 0.001 m XY tolerance
- [x] Buffer, planar: round/flat ends, dissolve NONE/ALL, negative polygon buffers;
      `BUFF_DIST`/`ORIG_FID` — matches ArcGIS shape within 0.2 m, area within 0.01%,
      point buffers exactly (ArcGIS arc densification rule reproduced)
- [x] Buffer `line_side` LEFT/RIGHT (one-sided strips, wedges where a bend opens on that
      side, quarter discs for ROUND ends) and OUTSIDE_ONLY (buffer minus the polygon),
      `dissolve_option` LIST (one multipart buffer per value combination, sorted), distance
      from a field — match ArcGIS (one-sided ROUND ends within 0.02% of area: ArcGIS's
      quarter arcs stop a fraction of a degree short of the line's axis)
- [x] Buffer: true arcs in geodatabase output, as ArcGIS keeps them (point buffers, dissolved or
      not; undissolved polygon buffers; line and dissolved polygon buffers stay densified, as in
      ArcGIS): `recover_arcs` finds the runs of vertices on each input vertex's circle; a point's
      circle is a two-vertex ring from the north pole through the south pole, trimmed arcs split
      at the north pole and keep the south pole as interior point; CCW flag on hole arcs. Matches
      ArcGIS's vertices and arcs (`testdata/gdb-reference/buffers.gdb`); open: where ArcGIS starts
      a polygon ring (its seam can split an arc), one 2° stretch of a 60 m dissolve kept as vertices
- [ ] Buffer: geodesic for geographic data
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
- [x] Spatial Join merge rules (First, Last, Join, Sum, Mean, Median, Mode, Min, Max,
      StdDev, Count, Range; `gf spatial-join --merge-rule FIELD=RULE[:DELIM]`): output types
      kept (integers rounded), numeric rules give text fields null, Count writes text, Mode
      ties go to the smallest value, sample standard deviation; Join text too long for its
      field empties that field and the rest of the row (WARNING 001156) — match ArcGIS
- [x] Spatial Join with a full field mapping string (`gf spatial-join --field-mapping`): output
      names, types (Long/Short/Float/Double/Text), lengths, text start/end positions, merge rules
      and several source fields per output (first non-listed-null source wins under First),
      values converted as ArcGIS does (doubles round to integers, text that is not a number
      reads as 0, numbers become text, text cut to the length) — matches ArcGIS; visible/alias
      are ignored for shapefile output as in ArcGIS
- [x] Merge with a field mapping (`gf merge ... --field-mapping`): output fields as mapped, each
      input contributing its own source field (found by table name), values converted to the
      output type — matches ArcGIS
- [x] Append with a field mapping (`--field-mapping`, NO_TEST): target fields take the first source
      belonging to the appended input, converted to the target's type — matches ArcGIS
- [x] Feature Class To Feature Class (`gf feature-class-to-feature-class`, where clause and field
      mapping) — matches ArcGIS
- [ ] Date outputs in field mappings; Merge/Append mappings with merge rules over several rows
- [ ] Split into a geodatabase (needs adding to an existing one); selection types other than
      NEW_SELECTION
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
- [x] Multipart To Singlepart, Feature To Point (CENTROID; INSIDE = arcpy's label point:
      lines nearest the envelope center, polygons the centroid or a horizontal-split fallback),
      Feature Vertices To Points (ALL, START/END/BOTH_ENDS and MID per part, DANGLE with
      DANGLE_LEN), Minimum Bounding Geometry (envelope, convex hull, rectangles by area/width,
      circle; NONE/ALL/LIST; MBG_FIELDS; straight lines widened 1 unit each side), Add XY
      Coordinates — match ArcGIS
- [x] Symmetrical Difference (`gf symmetrical-difference`): Union's one-input-only rows in its
      order, minus slivers narrower than the XY tolerance and vertices within it of their
      neighbours' line (spikes included) — matches ArcGIS. Union itself still keeps such slivers
- [x] Update (`gf update`, BORDERS): Erase's rows followed by the update polygons carrying the
      same-named fields — matches ArcGIS; NO_BORDERS not implemented
- [x] Sort (`gf sort in out "ZONE DESCENDING;VALUE ASCENDING"`): field keys (numbers by value, text by
      bytes, ties in input order) and `Shape` with UR/UL/LR/LL (the envelope corner's y then x),
      `ORIG_FID` added — matches ArcGIS on 12 cases
- [x] Delete Identical (`gf delete-identical`, in place or `--output`): first of each group kept in
      input order, FIDs closed up, "N duplicate(s) from M group(s) deleted." — matches ArcGIS
- [x] Add Field (TEXT/SHORT/LONG/FLOAT/DOUBLE/DATE with precision, scale, length; names fixed up with
      WARNING 000304; an existing name warns, ERROR 000012 for another type) and Delete Field
      (DELETE_FIELDS/KEEP_FIELDS; the last field stays, ERROR 002557) for shapefiles, in place or
      `--output` — match ArcGIS on 15 cases; Alter Field, Calculate Field still to do
- [x] Calculate Field (`gf calculate-field`, PYTHON3): a Python-subset interpreter (`pycalc`:
      arithmetic with Python semantics, comparisons, conditional expressions, str methods and
      slicing, `%` / `format` / f-string formatting, `math`, common builtins, a code block with
      `def`/`if`/`for`/`while`) with `!FIELD!` values; integers round half to even into integer
      fields, numbers become text, a failing or None row is ERROR 160145 for a shapefile; the
      field is created from `--field-type` if missing — matches ArcGIS on 28 expressions.
      Not covered: Arcade/SQL expressions, comprehensions, dictionaries, exceptions, imports
      besides `math`, date arithmetic
- [ ] Polygon To Line / Feature To Line: ArcGIS's row order is banded (holes first, then outer
      rings, row of parcels by row) in a way not worked out; LEFT_FID/RIGHT_FID semantics known
- [x] Select with SQL `where_clause` (comparisons, AND/OR/NOT, IN, LIKE, IS NULL, arithmetic,
      quoted names; BETWEEN rejected with ERROR 000358 as for shapefiles), Summary Statistics
      (sample STD/VARIANCE, unlike Dissolve's population ones; MEDIAN, UNIQUE) and Frequency
      into `.dbf` tables — match ArcGIS
- [x] Add Geometry Attributes (area, perimeter/length with units, centroid, inside point,
      extent, start/mid/end, bearing, part and point counts; in place), Points To Line (line
      and sort fields, close), Generate Near Table (CLOSEST or ALL with rank and count,
      location, angle), Find Identical (fields or Shape, ONLY_DUPLICATES) — match ArcGIS
- [x] Feature To Point INSIDE for any polygon: ArcGIS's label point reconstructed from its outputs
      (largest part; area centroid and the lowest chord midpoints of vertical lines at 1/2, 3/4,
      1/4 of the width, scored by boundary distance × rank weight; `docs/formats/label-point.md`)
      — matches ArcGIS on 298 random polygons incl. holes and multipart
- [ ] Label point of self-intersecting input that ArcGIS simplifies into holes touching the outer
      ring (two of 300 random polygons differ)
- [ ] Check/Repair Geometry: geodatabase output tables, curves, multipoint and Z/M
      problems ("empty Z values", "bad envelope", ...)
- [x] Clip/Erase against many polygons: each feature is clipped by the union of only the
      clip polygons whose envelopes reach it (unions cached per candidate set). 10k parcels
      by 10k parcels: 34 s → 0.05 s; by 300 road buffers: 30 s → 1.6 s; 20k wells: 1.6 s → 0.05 s
- [ ] Shapefile writer: `.shp.xml` (ArcGIS writes it for tool outputs; `.sbn/.sbx` done)

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
- [x] Table outputs (Frequency, Statistics, ...) into a new geodatabase: fields and rows equal
      ArcGIS's (`testdata/gdb-reference/stat_tables.gdb`)
- [x] Write: add to an existing geodatabase — tables, feature classes, domains: `GeodatabaseWriter::open_existing`
      appends rows to `GDB_SystemCatalog`, `GDB_SpatialRefs`, `GDB_Items` and `GDB_ItemRelationships`
      and rebuilds their indexes (the rebuild reproduces ArcGIS's `.atx` files byte for byte);
      `gf` tool outputs now go into an existing `.gdb`. Checked in ArcGIS: it lists, describes and
      reads the new items, creates further items in the same geodatabase, indexes, edits and compacts
- [x] Edit rows of an existing table or feature class in place (`TableEditor`: insert, update,
      delete; row space is appended, `.gdbtablx`, spatial index, extent and attribute indexes follow;
      ArcGIS reads, edits and compacts the result)
- [x] `rewrite_table`: replace the fields and rows of an existing table keeping its file number, geometry
      definition, chosen OBJECTIDs and surviving attribute indexes (the primitive for Add/Delete Field
      and in-place tools on `.gdb` data); not yet wired into `gf`
- [x] Editing in place from tools on `.gdb` data (`gf` in-place tools such as Add/Delete Field, Calculate
      Field, Append, Near, Delete Identical go through `rewrite_table`; OBJECTIDs the tool carries through
      are kept; `tests/gdb_in_place.rs`). Not yet re-verified in ArcGIS for every tool
- [x] Delete and Rename of tables, feature classes, relationship classes, feature datasets and domains
      (`delete_item`, `rename_item`, `delete_domain`, `rename_domain`; `gf delete/rename/delete-domain/
      alter-domain`): catalog, items, relationships, definition XML and files equal ArcGIS's after each
      operation (`testdata/gdb-reference/catalog_ops`, `scripts/gdb-snapshot.py`); ArcGIS reads, edits and
      compacts the results. A table's deletion takes its relationship classes with it; a domain fields use is
      refused (ERROR 002630)
- [ ] Delete and Rename of subtypes
- [ ] ArcGIS's `timestamps`, `.freelist` (space reuse) and `.horizon` files; 64-bit OBJECTID (v4) tables
- [ ] Write: per-vertex missing M values mixed with present ones (encoding unknown)
- [x] Write: attribute indexes on new tables (`TableBuilder::add_attribute_index`; all indexable field
      types; `.atx` and `.gdbindexes` byte-identical to ArcGIS's, also 300 000-row three-level
      trees) — the page layout (shared boundary children, bottom-up page numbers) is documented
- [x] Write: feature datasets (`create_feature_dataset`, `create_feature_class_in`; new or existing geodatabase;
      items, `\ds\name` paths, `DatasetInFeatureDataset` relationships and the dataset's registered
      coordinate system as ArcGIS writes them; classes take the dataset's XY domain; `gf create-feature-dataset`,
      outputs to `city.gdb/Ds/name`); ArcGIS lists, reads, edits and deletes in them. Not done: projecting an
      output to the dataset's coordinate system; ArcGIS's projection-derived XY domain for projected datasets
- [ ] Write: subtypes, relationship classes
- [x] Written geodatabases open in ArcGIS Pro 3.7: Describe, counts, domains, Check Geometry
      (no problems beyond those already in the source data), Copy Features out of them
- [ ] Projected coordinate systems: use ArcGIS's projection-derived XY domain origin (needs Phase 4)

## Phase 4 — CRS
- [x] PROJ binding (`geoforge-proj` on the `proj` crate, system PROJ >= 9.6.2 or built from
      source): CRSs from factory codes (EPSG, then Esri), `.prj` files or WKT; Esri WKT out
- [x] Project tool (`gf project in out 4326`): vertices transformed by PROJ, Z/M kept — matches
      ArcGIS to ~1e-13° / sub-mm on WGS 84 ↔ UTM / Web Mercator; output `.prj` is ArcGIS's
      regenerated WKT, which every tool now writes (ArcGIS does not copy the input's text)
- [x] Project `transform_method`: ArcGIS's transformation names (`A + B` chains, either direction)
      found through PROJ's EPSG/Esri aliases and applied between the geographic systems
      (`gf project --transform-method`); ED50 ↔ WGS 84 / ETRS89 Helmert methods match ArcGIS on
      geographic and projected data. Without a method PROJ's pick agreed with ArcGIS's default
      in the reference case
- [ ] Project: Esri-only methods PROJ has no alias for (e.g. `ED_1950_To_WGS_1984_NGA_7PAR`),
      ArcGIS's default method choice per area, vertical CS
- [x] Datum transforms with grid shifts: `NAD_1927_To_NAD_1983_NADCON` (alone and chained with
      `NAD_1983_To_WGS_1984_1`) match ArcGIS to ~1e-9° once PROJ has the grid; no grids are
      bundled, PROJ fetches them with `PROJ_NETWORK=ON` (the reference cases run only then)
- [x] A missing datum grid is named in the error (and the `PROJ_NETWORK=ON` / `PROJ_DATA` remedy)
- [ ] Grid-shift methods beyond NADCON checked against ArcGIS (NTv2, HARN, Alaska)

## Phase 5 — Raster
- [ ] GeoTIFF/COG read/write (tiling, compression, overviews)
  - [x] Reader: strips and tiles, none/LZW/Deflate/PackBits with predictors 2 and 3, both byte
        orders, BigTIFF, planar and pixel-interleaved bands, PixelIsPoint, GDAL_NODATA, EPSG
        GeoKeys (own chunk decoder; the `tiff` crate's LZW rejects GDAL's small tiles), checked
        cell for cell against GDAL
  - [x] Writer (own encoder): strips or tiles, all bands, none/LZW/Deflate/PackBits (rows
        packed separately), horizontal and floating-point predictors (LZW/Deflate only, as
        libtiff), internal overviews (nearest/average), BigTIFF, EPSG GeoKeys and GDAL_NODATA;
        checked with GDAL and read by ArcGIS; tool output uses ArcGIS's 128×128 tiles
  - [x] Reader keeps stored statistics (GDAL_METADATA tag, .aux.xml) — ArcGIS's local tools
        choose output types from them
  - [x] `.aux.xml` statistics beside every Spatial Analyst output, as ArcGIS writes them (count,
        min, max, mean, sample standard deviation, `%.14g` numbers; none for Euclidean Distance,
        Fill, Flow Direction, Reclassify, Stream Link, Watershed, Zonal Statistics; Basin writes
        count 0), so chained tools pick the same output types; checked on all 167 references
  - [ ] `.aux.xml` histograms. Found so far: the bucket is `floor((v - min) / (max - min) *
        (n - 1) + 0.001)` for 256-bucket float histograms (exact on 14 of 14 float outputs);
        integer histograms have one bucket per value (bounds min..max, or half-integer bounds
        for resample/streamlink/watershed), but several tools (slope, times, zonal means, ...)
        follow other rules not yet worked out
  - [ ] COG layout (overviews before the full image, ghost area), user-defined CRS GeoKeys
- [ ] Map algebra (local/focal/zonal/global), reclass, resample
  - [x] Focal Statistics (rectangle/circle/annulus in cells; mean, majority, maximum, median,
        minimum, minority, range, std, sum, variety; DATA/NODATA) matching ArcGIS: radii truncate
        to whole cells, the annulus is inclusive at both ends, NODATA treats cells beyond the edge
        as NoData, majority/minority ties go to the center value, NoData is the output type's
        maximum, Variety writes a 4-bit raster (NoData 15) for 3×3, integer Sum is 32-bit
  - [x] Majority/minority ties not involving the center go to the smallest value (checked on
        random classes, rectangle and circle, DATA and NODATA)
  - [x] Reclassify (values and inclusive ranges, NODATA, DATA/NODATA for missing values) and
        Zonal Statistics as a raster (zones on the value grid; ten statistics; DATA/NODATA)
        matching ArcGIS, including its integer output type choice: from 8-bit input a 1/2/4-bit
        layout (NoData 1/3/15), then signed, then unsigned 8-bit; otherwise U16 for non-negative
        results, else S16, then 32-bit; zonal Sum/Mean/Std are float
  - [x] Local tools matching ArcGIS: Plus, Minus, Times, Divide (raster or constant operands;
        integer inputs give 32-bit integers, Divide truncating, ÷0 NoData), Int, Float, Is Null,
        Con and Set Null with `Value` where clauses
  - [x] Zonal Statistics as Table (.dbf): zones in order of first appearance, median of the middle
        pair, PCT90 linearly interpolated, integer-only VARIETY/MAJORITY/MINORITY with counts and
        percentages rounded to 4 decimals, matching ArcGIS
  - [x] Zones and values on different grids, as ArcGIS lines them up: the value raster's cells
        over the intersection of both extents (widened to whole value cells), zones sampled at
        cell centers; integer medians of even counts round down; Range from 16-bit input that
        fits is unsigned 8-bit
  - [x] Extract by Mask with the mask on any grid: the larger cell size, aligned to the input
        over the intersection of both extents, both sampled at cell centers; integer output
        takes ArcGIS's type choice (16-bit input → U16, 8-bit → sub-byte layouts), float
        NoData −FLT_MAX, matching ArcGIS
  - [x] Local math on any grids and ArcGIS's integer types: two rasters meet on the
        intersection of their extents in the larger cells, anchored at its lower-left corner;
        with stored statistics (GDAL metadata or .aux.xml) on every raster the type fits the range
        the statistics and constants allow (1/2/4-bit, 8-bit, ...), otherwise it follows the
        operand types (U8 + U8 → U16, U8 − U8 → S16, S16 + U8 → S32; Divide keeps the wider type,
        or the next signed one); integer Divide rounds down; Int keeps or narrows the type
  - [x] Con/Set Null on any grids with the same type rule over the true/false values; Con's
        grid comes from the condition and true rasters only (the false raster is sampled onto
        it), Set Null's from the condition and false rasters, matching ArcGIS
  - [ ] Cell size/snap/extent/mask environments
- [ ] Hillshade, slope, aspect, curvature, contours, viewshed, hydrology (fill/flow dir/accumulation/watershed)
  - [x] Slope (degree/percent rise, z-factor), Aspect, Hillshade (no shadows), planar, matching
        ArcGIS cell for cell: Horn's gradient; border cells and cells with a NoData edge neighbor
        are NoData; a NoData corner drops out of its row/column weighted mean (Hillshade instead
        needs all 8); Hillshade truncates to 0–254 with NoData 255; float output NoData is −FLT_MAX,
        or 3.4e38 when the input has NoData; Aspect −1 on flat cells
  - [x] Hillshade shadows: ray toward the sun sampled bilinearly cell by cell, past NoData
  - [x] Viewshed (approximate): observer counts per cell, OFFSETA default 1 scaled by the z-factor,
        sight lines sampled cell by cell along the major axis; about 1% of cells differ from
        ArcGIS where sight lines graze the terrain (no exact model found: tried boundary and
        center-line crossings, bilinear sampling, Earth curvature)
  - [ ] Viewshed exactly as ArcGIS; OFFSETB, RADIUS and other observer fields
  - [ ] Hillshade shadows exactly as ArcGIS (3 and 29 cells near the horizon differ in the tests)
  - [ ] Geodesic method, tiled output with statistics (.aux.xml) like ArcGIS
  - [x] Curvature (with profile and plan outputs), identical to ArcGIS: Zevenbergen–Thorne with
        NoData and off-edge neighbors taking the center value
  - [x] Contour matching ArcGIS (float and integer surfaces, NoData): four triangles per square
        of cell centers around the corner mean, a cell equal to the contour value counts as below,
        exactly collinear vertices dropped, a square with one NoData corner keeps the triangles
        away from it, higher ground on the right, ordered by value descending then by length; Id
        and Contour fields
  - [ ] Contour: closed rings start at another vertex than ArcGIS's (shapes are equal)
  - [x] Fill (priority flood from the edge and NoData, z-limit per sink) and Flow Accumulation
        (D8, FLOAT/INTEGER, weights) matching ArcGIS exactly
  - [x] Flow Direction D8 (NORMAL/FORCE): steepest descent, single-cell pits raised first, ties
        to the larger code (two) or the middle of the arc (three), corners flow diagonally out
  - [x] Basin (touching outlets form one basin, numbered in raster order), Watershed (raster
        pour points), Stream Link (stretches numbered in raster order of their heads), Stream
        Order (Strahler, Shreve) and unweighted Flow Length (downstream/upstream), matching ArcGIS
  - [x] Raster To Point and Raster To Polygon (NO_SIMPLIFY; 4-connected regions in raster order of
        their first cell) matching ArcGIS
  - [x] Feature To Raster, Polygon To Raster (CELL_CENTER, MAXIMUM_AREA) and Point To Raster
        matching ArcGIS: grid anchored at the extent's lower-left corner (points and lines widened
        by half a cell), round(extent / cell) cells; points on a cell edge go left/up; lines mark
        cells whose interior they cross, most length wins; MAXIMUM_AREA leaves cells NoData when
        the uncovered part is larger; integer fields use ArcGIS's sub-byte/8-bit type choice
  - [x] GeoTIFF reader: 1/2/4-bit samples (ArcGIS writes them)
  - [x] Resample (nearest, bilinear, cubic a = −0.5 with out-of-raster taps dropped and weights
        renormalized; majority over the 4 × 4 input cells nearest the output cell center,
        clipped to the output extent, ties to the smaller value, NoData where the center cell
        is — one cell at a NoData hole still differs), Euclidean Distance (exact, max distance)
        and Extract by Mask, matching ArcGIS
  - [x] Point To Raster / Feature To Raster on points: MOST_FREQUENT ties go to the first
        feature; integer fields stay 32-bit (polygons and lines narrow them), COUNT is signed
        8-bit, MEAN of integers single; RANGE and STANDARD_DEVIATION (sample, single, NoData for
        one point), matching ArcGIS
  - [x] Polyline To Raster (MAXIMUM_LENGTH, MAXIMUM_COMBINED_LENGTH; its grid starts at the
        lines' lower-left corner with floor(extent / cell) + 1 cells, unlike Feature To Raster)
        and Polygon To Raster MAXIMUM_COMBINED_AREA, matching ArcGIS; sub-byte results whose
        values run exactly from 1 to the layout's top keep NoData 0
  - [ ] Raster To Polygon SIMPLIFY: rule worked out for blocky rasters (104 of 122 test rasters exact;
        arcs between nodes, corners displaced diagonally by a from a fixed grid, Douglas-Peucker-like
        split with collapse/clamp, fewest-vertices choice of a) — see `docs/formats/raster-to-polygon-simplify.md`;
        open (parked after two more agent rounds, 118/139 rasters, realistic rasters 0-40%): which corners are kept on
        smooth curves (circles, ellipse, real DEM classes), float ties in the choice of a; best untested idea: Potrace-like
        fewest-lines search between displaced corners constrained to stay within a; steps 1-6, 8-11 of notes.md are portable. Original observation: SIMPLIFY is not vertex weeding: a 7-step 30 m staircase from (210, 0)
        to (0, 210) becomes (210, 0) → (118.519, 118.519) → (0, 210), bulging off the cell
        corners (longer staircases: 221.812 for 14 steps, 327.009 for 21, 428.4 for 28); shared
        edges stay identical between neighbors
  - [ ] Flow Direction on flats: ArcGIS's flat resolution is approximated (5×5 look-ahead, then
        Jenson–Domingue passes preferring edge neighbors); a few cells differ on float DEMs and
        about 12% on an integer DEM's large flats

## Phase 6 — Toolbox framework
- [ ] Tool trait: typed parameters, validation, progress, cancellation, geoprocessing history
- [ ] Port tools by toolbox (Analysis → Data Management → Spatial Analyst → …)
- [ ] Python bindings via PyO3 (`geoforge` module, arcpy-like API)

## Phase 7 — Databases & other formats
- [x] Features To JSON: Esri JSON and GeoJSON written byte for byte as ArcGIS writes them
      (`geoforge-json`: `%.17g` coordinates, field-scale-dependent number forms, formatted
      header with a one-line features array, wkid/latestWkid through PROJ's identify)
- [x] JSON To Features: Esri JSON (field types kept, the OID becomes an empty field,
      Shape_Length/Shape_Area added at 0 as ArcGIS's geodatabase detour leaves them) and GeoJSON
      (one geometry type, numbers as doubles, text(254), always 3D) — match ArcGIS
- [x] CSV (`geoforge-csv`): tables read with ArcGIS's type inference (Long / Double /
      Text(254), usable by every tool), Export Table to `.csv` byte-identical to ArcGIS's (BOM,
      CRLF, `%.15f` doubles, RFC 4180 quoting), XY Table To Point (Z field, coordinate system)
- [ ] GeoPackage / SpatiaLite, PostGIS, KML, GML, DXF, E00, ERDAS IMG, JPEG2000
  - [x] Own read-only SQLite reader (`geoforge-gpkg::sqlite`): header, table b-trees, overflow
        pages, records with type affinity, CREATE TABLE columns and rowid aliases; checked row
        for row against Python's sqlite3 at page sizes 512, 4096 and 65536
  - [x] GeoPackage feature and attribute tables read as ArcGIS reads them (`main.` names, 64-bit
        ObjectID, SQLite types → ArcGIS field types, Esri ring orientation keeping start
        vertices, empty points), usable as input to every `gf` tool; tools that ArcGIS refuses
        to run from 64-bit ObjectIDs into shapefiles fail the same way
        (`docs/formats/geopackage.md`)
  - [x] Shapefile output: Date Only fields fail with ERROR 000308; empty points written as
        −DBL_MAX as ArcGIS does
  - [x] Own SQLite writer (`geoforge-gpkg::sqlite_writer`): bulk-loaded table and index
        b-trees, overflow pages, automatic indexes, schema beyond page 1, lock-byte page; passes
        `PRAGMA integrity_check` at page sizes 512–65536 and SQLite keeps editing the files;
        2 million rows in 0.18 s (≈ 8× SQLite's own bulk insert), with an index 0.53 s (≈ 3×)
  - [x] GeoPackage writing as ArcGIS does (`gf <tool> ... out.gpkg/name`): ArcGIS's schema for a
        new file, feature tables with its column definitions and checks, OBJECTID from 1,
        little-endian blobs without envelope (MULTIPOLYGON/MULTILINESTRING, OGC ring order),
        coordinates snapped to ArcGIS's XY domain, Esri WKT and WKT2 spatial reference rows,
        R*Tree spatial index bulk-loaded (passes `rtreecheck`) with ArcGIS's triggers; equal
        row for row to ArcGIS's Copy Features output; ArcGIS reads, queries and edits the
        files. Adding a table rewrites the file. 1 million points in 0.5 s (GDAL: 1.3 s)
  - [x] ArcGIS XY domains: geographic, Mercator variants, Plate Carrée exact, Transverse
        Mercator's y origin exact (x approximate)
  - [x] Export Table into a GeoPackage (`gf export-table in out.gpkg/name`): attribute tables
        equal row for row to ArcGIS's (`testdata/gpkg/arcgis_tables.gpkg`)
  - [x] Table outputs (Frequency, Statistics, Summary/Near tables, ...) into a GeoPackage:
        Frequency and Statistics tables equal ArcGIS's (`testdata/gpkg/arcgis_stat_tables.gpkg`)
  - [ ] GeoPackage: WAL files, curve geometries, Project's extra `fid_` field,
        coordinate system descriptions from ArcGIS's database; Transverse Mercator x origin and
        other projections' XY domains (also for file geodatabase output); 64-bit ObjectIDs in
        file geodatabases

## Phase 8 — Rendering & UI
- [ ] wgpu-based 2D map renderer (symbology, labeling, raster stretch)
- [ ] Desktop app shell: `app/` (vendored RustGIS, egui) is the shell. File Geodatabase `FeatureSource`
      and File Geodatabase output done (`app/crates/data/src/source/filegdb.rs`), our overlay/proximity tools
      registered in its `ToolSpec` registry (`app/crates/geoforge`); next: more tools, Python/MCP bindings
- [ ] Layout view + PDF/PNG/TIFF export; 3D scene view

## Phase 9 — Advanced
- [ ] Network analysis, geocoding, spatial statistics, geostatistics, image classification, point clouds (LAS/LAZ)
