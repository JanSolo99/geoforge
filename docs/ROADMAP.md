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
- [x] Buffer: geodesic for geographic data (`BufferDistance::Geodesic`, `geoforge_geometry::buffer_geodesic`; `gf buffer
      in out "1 Kilometers"` on a geographic system; a plain number or degrees stays planar in degrees, as in
      ArcGIS). Points are the exact geodesic circle on WGS 84 (starts north, clockwise, 223 steps at 1 km) with
      ArcGIS's distance-tiered chord deviation; lines and polygons are built from the planar pieces with every offset
      found by the direct geodesic problem, their arcs on a global azimuth grid (10°/2^k, anchors at multiples of
      10°, partial stretches by the planar rule for lines and by grid steps for polygons). Equal to ArcGIS's
      vertex for vertex (points, lines to 5 km, flat/left/right, closed) or within 0.05 m of its boundary
      (polygons, holes, negative, outside-only) on 25 cases (`testdata/arcgis-reference/geodesic_buffers.json`);
      0.8 m at a sharp bend at 65 N; beyond 50 km ArcGIS's line arcs differ in a way matched only to their 100 m
      deviation. Refused: buffers over the antimeridian or a pole (the app falls back to a UTM zone)
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
  - [x] Geodesic Near / Generate Near Table (`--method GEODESIC`) and Spatial Join `CLOSEST_GEODESIC` /
        `WITHIN_A_DISTANCE_GEODESIC` on longitude/latitude data (`geoforge-geometry::nearest_geodesic`; metres on WGS 84,
        azimuth angles): equal to ArcGIS between points and for polygons; ArcGIS finds the closest point on a plain line
        only approximately, so line cases differ by up to 0.08 % in distance (`tests/near_geodesic.rs`, 54 cases). Segments
        are straight in longitude/latitude, so pairs across the antimeridian are not handled. The app's Near, Generate
        Near Table and Spatial Join use it (Select By Location distances still use the UTM zone).
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
- [x] Edits are crash-safe transactions: a rollback journal per geodatabase (`.gf-journal`: originals moved
      aside, appended tables' length and header kept, synced before each change; commit = deleting the
      manifest; any later open rolls an unfinished edit back byte for byte). Catalog operations that touch
      several system tables are one transaction; a dropped editor or a failing rewrite rolls back.
      `tests/crash_safety.rs` kills a child process at every step of five kinds of edit (8 to 58 steps each)
- [x] Edits on geodatabases ArcGIS wrote (`testdata/gdb-reference/edit_arcgis.gdb`): `TableEditor::get`,
      `set_value`, `set_geometry`; `Shape_Length` / `Shape_Area` computed from the stored geometry; 64-bit
      OBJECTID (v4) tables edited and rewritten (v4 header and `.gdbtablx`, version 2 indexes); `add_field`,
      `delete_fields`, `delete_rows`, `truncate_table`; rewrites keep the OBJECTID sequence and ArcGIS's index
      names. GDAL reads every edited table as we do; ArcGIS reads, queries (spatial and attribute index), checks,
      compacts and edits further, and `gf delete-rows/delete-features/truncate-table/add-field/calculate-field/
      delete-field` give the same fields, rows, indexes and next OBJECTIDs as the same ArcGIS tools
- [x] `rewrite_table`: replace the fields and rows of an existing table keeping its file number, geometry
      definition, chosen OBJECTIDs and surviving attribute indexes (the primitive for Add/Delete Field
      and in-place tools on `.gdb` data)
- [x] Editing in place from tools on `.gdb` data (`gf` in-place tools such as Add/Delete Field, Calculate
      Field, Append, Near, Delete Identical go through `rewrite_table`; OBJECTIDs the tool carries through
      are kept; `tests/gdb_in_place.rs`, `tests/gdb_edit_rows.rs`). Add/Delete/Calculate Field checked against
      ArcGIS; the other in-place tools not yet
- [x] Delete and Rename of tables, feature classes, relationship classes, feature datasets and domains
      (`delete_item`, `rename_item`, `delete_domain`, `rename_domain`; `gf delete/rename/delete-domain/
      alter-domain`): catalog, items, relationships, definition XML and files equal ArcGIS's after each
      operation (`testdata/gdb-reference/catalog_ops`, `scripts/gdb-snapshot.py`); ArcGIS reads, edits and
      compacts the results. A table's deletion takes its relationship classes with it; a domain fields use is
      refused (ERROR 002630)
- [x] Remove subtypes (`remove_subtype`; ArcGIS has no rename operation)
- [ ] ArcGIS's `timestamps`, `.freelist` (space reuse) and `.horizon` files (64-bit OBJECTID tables: done)
- [ ] Write: per-vertex missing M values mixed with present ones (encoding unknown)
- [x] Write: attribute indexes on new tables (`TableBuilder::add_attribute_index`; all indexable field
      types; `.atx` and `.gdbindexes` byte-identical to ArcGIS's, also 300 000-row three-level
      trees) — the page layout (shared boundary children, bottom-up page numbers) is documented
- [x] Write: feature datasets (`create_feature_dataset`, `create_feature_class_in`; new or existing geodatabase;
      items, `\ds\name` paths, `DatasetInFeatureDataset` relationships and the dataset's registered
      coordinate system as ArcGIS writes them; classes take the dataset's XY domain; `gf create-feature-dataset`,
      outputs to `city.gdb/Ds/name`); ArcGIS lists, reads, edits and deletes in them. `gf` projects an output to
      the dataset's coordinate system, and the class takes ArcGIS's own XY domain for that system
- [x] Write: subtypes and field defaults (`create_table_with_subtypes`; subtype field, default subtype,
      per-subtype domains and defaults; the `Subtypes` element equals ArcGIS's character for character, the subtype
      field takes the default code as its default, domains link to the table) and simple relationship
      classes (`add_relationship_class`: one to one / one to many, composite, notification, labels, keys; items,
      links and Describe equal ArcGIS's; GDAL lists them). ArcGIS reads all of it. The reader now resolves subtype
      defaults against the table's real field types (ArcGIS lists only some fields in the definition)
- [x] Write: attributed and many-to-many relationship classes (own table `<key>, <key>, RID`, keys indexed
      `GDB_<DSID>_<field>`; reader-level equal to ArcGIS's; ArcGIS describes and deletes them). Needs the four
      keys (ArcGIS derives names when the destination keys are left out: not reproduced)
- [x] Edit subtypes, field domains and defaults of existing tables (`set_subtype_field`, `add_subtype`,
      `remove_subtype`, `set_default_subtype`, `assign/remove_domain_from_field`, `assign_default_to_field`;
      `gf set-subtype-field` ... `gf assign-default-to-field`): definition XML, domain links and the table's own
      defaults equal ArcGIS's after 12 operations (`testdata/gdb-reference/subtype_ops`); ArcGIS applies them
- [x] Written geodatabases open in ArcGIS Pro 3.7: Describe, counts, domains, Check Geometry
      (no problems beyond those already in the source data), Copy Features out of them
- [x] Projected coordinate systems: ArcGIS's XY domain (origin, resolution, tolerance) for every system PROJ
      knows (7,135 by Esri name), collected from ArcGIS through the bridge (`scripts/make-xy-domains.py`,
      `xy_domains.tsv.gz`); used by file geodatabase and GeoPackage output. The origin comes from ArcGIS's own
      projection engine, so it is a table, not a formula; a system ArcGIS does not know keeps the fixed origin

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
  - [x] Cell size/snap/extent/mask environments (`geoforge-raster::env`, matched cell-for-cell to ArcGIS on 23 runs in
        `tests/raster_env.rs`; observed rules in the module docs; app tools take `cell_size`/`extent`/`snap_raster`/`mask`,
        `arcpy.env.cellSize/extent/snapRaster/mask`; Euclidean Distance, Extract By Mask, Zonal and the vector conversions ignore them)
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
  - [x] Euclidean Direction (whole-degree azimuth, 360 due north, 0 on sources, 16-bit, NoData 32767)
        and Euclidean Allocation (nearest source's value, ArcGIS's integer type choice, max
        distance), matching ArcGIS on every cell, including cells equally near to several
        sources: ArcGIS's pick is reproduced by its vector-propagation order (rows from east
        then west; top-down taking NE, N, NW with ties replacing; bottom-up from SW, S, SE, W;
        then from E), checked on 4,600+ ties; the allocation's value attribute table is not
        written
  - [x] Kernel Density (points and lines; default radius by Silverman's rule, weighted
        by the population field, area units, EXPECTED_COUNTS), Point Density (circle, rectangle,
        annulus, wedge; map or cell units) and Line Density, all matching ArcGIS exactly in 13
        cases (`gf kernel-density|point-density|line-density`). Point Density's grid starts
        (w + h) / 798 before the points' corner. Not done: barriers, GEODESIC method (planar
        only), map units other than meters, the app tools and arcpy entries
  - [x] Extract Values to Points (`gf extract-values-to-points`; plain and INTERPOLATE on integer, float
        and NoData rasters, edges, shapefile NoData as -9999 or 0), 7 cases exact. ArcGIS writes the
        points grouped by spatial tile (order engine-internal; the probe cases compare by PID). Not
        done: ADD_ATTRIBUTES ALL joining a raster attribute table, geodatabase NoData (assumed null),
        the app tool and arcpy entry, points in another coordinate system than the raster
  - [x] Point To Raster / Feature To Raster on points: MOST_FREQUENT ties go to the first
        feature; integer fields stay 32-bit (polygons and lines narrow them), COUNT is signed
        8-bit, MEAN of integers single; RANGE and STANDARD_DEVIATION (sample, single, NoData for
        one point), matching ArcGIS
  - [x] Polyline To Raster (MAXIMUM_LENGTH, MAXIMUM_COMBINED_LENGTH; its grid starts at the
        lines' lower-left corner with floor(extent / cell) + 1 cells, unlike Feature To Raster)
        and Polygon To Raster MAXIMUM_COMBINED_AREA, matching ArcGIS; sub-byte results whose
        values run exactly from 1 to the layout's top keep NoData 0
  - [x] IDW (Spatial Analyst interpolation, ported from RustGIS and rebuilt to ArcGIS's rules in
        `geoforge-raster::interpolate`; `gf idw`, the app's IDW tool, `arcpy.sa.Idw` with
        `RadiusVariable`/`RadiusFixed`): grid cell centers start at the points' lower-left corner
        (`round(w / cell) + 1` cells; a snap raster snaps the points' own extent outward instead);
        default cell = shorter side / 250; variable radius = the n nearest points but never beyond
        the output grid's width + height; fixed radius widens a cell size at a time until
        `min_points` are inside; default fixed distance 5 cells; ~1.5k cells over 8 searches,
        3 environments and 15 reach cases match Pro 3.7 (`testdata/arcgis-reference/idw.json`,
        probes in `scripts/research/idw/`). Barriers not done. Next in this family: Natural
        Neighbor, Spline, Kriging, Trend, Topo to Raster
  - [x] Overlay and local tools over several rasters (`geoforge-raster::overlay`; `gf cell-statistics`,
        `weighted-sum`, `highest-position`, `combine`, `fuzzy-membership`, `fuzzy-overlay`; app tools
        incl. Weighted Overlay; `arcpy.sa.CellStatistics`, `WeightedSum`, `HighestPosition`,
        `LowestPosition`, `Combine`, `WeightedOverlay`, `FuzzyMembership`, `FuzzyOverlay` and the
        Fuzzy*/WSTable/WOTable/RemapValue classes), every cell and output type matching Pro 3.7
        (`testdata/arcgis-reference/overlay.json`, probes in `scripts/research/overlay/`): DATA/NODATA
        handling, population STD, ties of MAJORITY/MINORITY to the smallest value, MEDIAN of an even
        count (average in float, lower middle in integer output), Combine numbering by first
        appearance, Weighted Overlay rounding half up to S8, the seven fuzzy functions and hedges,
        Fuzzy Overlay AND/OR/PRODUCT/SUM/GAMMA. Integer outputs are typed for the range the inputs'
        statistics allow (not the values found), values 0..1 giving NoData 16, wide signed input
        staying S32. Not done: Weighted Overlay RESTRICTED, value fields other than VALUE, the
        Combine/Weighted Overlay attribute tables, `gf weighted-overlay`
  - [x] Construction tools on the shared registry (`geoforge-tools::construct`): Densify, Generate
        Points Along Lines, Create Fishnet and Polygon To Line (IGNORE_NEIGHBORS), 24 runs matching
        Pro 3.7 (`testdata/arcgis-reference/construct.json`, probes in `scripts/research/construct/`):
        a segment longer than the distance is cut into floor(length / distance) + 1 equal parts (one
        no longer than it stays whole); points along lines walk the parts end to end, strictly
        before the total length, END_POINTS adding the first and last vertex; the fishnet's x-axis
        is the y-axis turned a quarter clockwise, polylines list the rows' lines then the columns'.
        Not done: IDENTIFY_NEIGHBORS (needs a planar graph), fishnet labels, Densify's ANGLE and
        OFFSET methods and curves, chainage fields, Multiple Ring Buffer and Create Thiessen
        Polygons (probed: `scripts/research/construct/probe1.py`)
  - [x] Table tools on the shared registry (`geoforge-tools::tables`): Copy Rows, Table To Table
        (where clause), Join Field (all or named fields, `_1` renaming, first match, nulls where
        unmatched; features or tables), Add Fields and Get Count matching Pro 3.7 on 9 runs
        (`testdata/arcgis-reference/tables.json`, `crates/geoforge-cli/tests/tables.rs`). The
        registry now has table results (`Outcome::Table`). Not done: Delete Rows and Alter Field (they
        edit their input; shapefiles cannot rename fields), field mappings, GDB table outputs from
        `gf run`
  - [x] Spatial Analyst math functions (`geoforge-raster::mathfn`; `gf math-fn`, 45 app tools,
        `arcpy.sa.Abs` ... `BitwiseRightShift`): Abs, Exp, Exp2, Exp10, Ln, Log2, Log10, Square Root,
        Square, Negate, the trigonometric and hyperbolic functions and their inverses, Round Down/Up,
        Boolean Not, Bitwise Not, Power, Mod, ATan2, the six relational and three Boolean operators,
        Diff, Over and the bitwise operators matching Pro 3.7 on 125 runs, values, output types and
        NoData (`testdata/arcgis-reference/mathfn.json`): float results are NoData outside the
        domain or beyond a 32-bit float; integer results are typed for the range the statistics
        allow; Mod is fmod; shifts are 32-bit and wrap. Not done: Plus/Minus/Times/Divide type rules
        beyond the earlier ones (see `local::math_type`), Negate/Abs on constants
  - [x] Aggregate, Block Statistics, Slice (EQUAL_INTERVAL), Pick and Region Group (FOUR, WITHIN)
        (`geoforge-raster::blockops`; app tools and `arcpy.sa`) matching Pro 3.7 on 82 runs, grids,
        types, NoData and values (`testdata/arcgis-reference/blockops.json`): Aggregate counts blocks
        from the lower-left corner, Block Statistics from the upper-left; an integer raster's range counts
        one wider for Slice. Not done: Slice's other methods (equal area differs on a cell, natural
        breaks), `gf` commands for Aggregate, Block Statistics, Slice and Pick
  - [x] Expand, Shrink (MORPHOLOGICAL and DISTANCE), Majority Filter (FOUR/EIGHT, MAJORITY/HALF) and
        Region Group (FOUR/EIGHT, WITHIN/CROSS, excluded value) (`geoforge-raster::generalize`; `gf
        expand|shrink|majority-filter|region-group`, app tools, `arcpy.sa`) matching Pro 3.7 cell for cell
        on 34 reference cases (`expand_*`, `shrink_*`, `majority_*`, `regiongroup_*`) and on synthetic
        probes: number of cells rounds half to even; morphological passes are synchronous with ties to
        the first value reading the neighbors column by column; DISTANCE ties go to the smallest value;
        Majority Filter is one pass, NoData is a value, edges mirror, contiguous runs on the ring with
        ArcGIS's walk order for ties; Region Group numbers by a union-find (union by label count) rather
        than reading order; output types fit the input range (Region Group: 1 ..= (rows+1)(cols+1)).
        Not done: Boundary Clean, Thin, the Region Group LINK attribute table
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
- [~] Tool registry: the app's `ToolSpec` registry (`app/crates/geoforge`) holds ~65 ArcGIS-verified tools with typed
      parameters (layer, field, number, choice, text, bool), required/default checking, a hostile-parameter test over
      every tool, and geoprocessing history/undo from the RustGIS engine. Not yet: progress and cancellation for long
      runs, environments (cell size, extent, mask, snap) as first-class settings
- [~] Port tools by toolbox: Analysis (Clip, Erase, Intersect, Union, Identity, Update, Symmetrical Difference, Buffer, Near,
      Spatial Join, Select, Split, Frequency, Statistics), Data Management (fields, features, geodatabase schema tools via
      `gf`), Spatial Analyst (surface, hydrology, focal, local, zonal, reclass, distance, extraction) done; the rest of the
      2,157-tool inventory is not
- [x] Python bindings: the app's PyO3 package and MCP server dispatch through the registry (`gp.run` with
      `geoforge:<tool>`), and `rustgis.arcpy` gives it ArcGIS's shape (`arcpy.analysis.Buffer("Cities", "out", "100 Kilometers")`,
      `management`, `conversion`, `sa`; inputs as layer names or paths, outputs written to paths; 10 tests)

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
        coordinate system descriptions from ArcGIS's database; 64-bit ObjectIDs in
        file geodatabases

## Phase 8 — Rendering & UI
- [ ] wgpu-based 2D map renderer (symbology, labeling, raster stretch)
- [ ] Desktop app shell: `app/` (vendored RustGIS, egui) is the shell. File Geodatabase `FeatureSource`
      and File Geodatabase output done (`app/crates/data/src/source/filegdb.rs`), our overlay/proximity tools
      registered in its `ToolSpec` registry (`app/crates/geoforge`, vector and raster tools); next: more tools
- [ ] Layout view + PDF/PNG/TIFF export; 3D scene view

## Phase 9 — Advanced
- [ ] Network analysis, geocoding, spatial statistics, geostatistics, image classification, point clouds (LAS/LAZ)
  - [ ] Spatial Statistics (0 of 63 at the start of this item)
    - [x] Measuring Geographic Distributions (`geoforge-tools::spatstat`; `gf mean-center`,
          `median-center`, `central-feature`, `standard-distance`, `directional-distribution`,
          `directional-mean`) matched to Pro 3.7 on 37 runs (two point sets, one line set; weights,
          case fields, dimension field, self potential, 1-3 deviations, orientation-only;
          `testdata/arcgis-reference/spatstat.json`, probes in `scripts/research/spatstat/`):
          schemas, case ordering, 361-vertex circles and ellipses, the Rayleigh statistic
          `2Z(1 + (Z-2)/4n)` with p = exp(-Z). Open: Median Center's stopping rule (Weiszfeld from the
          mean; ArcGIS takes the same steps but stops at a different one, up to ~10 m apart), the
          app tools and `arcpy.stats`
    - [x] Analyzing Patterns / Mapping Clusters: Average Nearest Neighbor (minimum enclosing
          rectangle area, Euclidean and Manhattan), Spatial Autocorrelation (Global Moran's I,
          randomization variance; inverse distance, squared, fixed band, zone of indifference
          1/(1+(d-band)), 8 nearest neighbors; row standardization) and Hot Spot Analysis (Gi*; the
          feature counts itself at weight 1, row standardization changes nothing; bins by p-value
          0.01/0.05/0.10; BH false discovery rate) matching Pro 3.7 on 2 point sets x 44 runs
          (`testdata/arcgis-reference/spatstat_patterns.json`, `crates/geoforge-cli/tests/spatstat_patterns.rs`).
          Tools that give values rather than features return a `registry::Report` (`gf run` prints
          it, the app shows a table and messages, arcpy `getOutput(i)`); ArcGIS's p-values are a
          normal-CDF approximation up to 2e-8 from the exact ones we use. Not done: polygon
          contiguity conceptualizations, spatial weights files, distances under 1 (treated as 1
          for inverse distance, unchecked), FDR checked only where nothing is significant
    - [ ] Next: Generate Spatial Weights Matrix, Collect Events, Multi-Distance Spatial Cluster
          Analysis (Ripley's K), Cluster and Outlier Analysis (random permutations), OLS and GWR
