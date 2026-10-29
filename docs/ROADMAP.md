# Roadmap

The verified inventory of ArcGIS Pro's tools, formats and arcpy API (from the official documentation,
2,157 tools) is in [`arcgis-inventory/`](arcgis-inventory/README.md); its `hard_items.md` collects the
documented algorithms of the hard-to-replicate tools.

See [`GAP_ANALYSIS.md`](GAP_ANALYSIS.md) (and `gap_list.csv`) for everything ArcGIS Pro has that
UniGIS does not yet, with difficulty, verifiability against ArcGIS, a recommended order and an
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

Built in thin vertical slices: each phase ends with something usable from `ug`
and verified against GDAL/OGR output on real data.

## Phase 0 — Foundations
- [x] Cargo workspace, crate layout
- [x] CI gate (fmt, clippy, test) runs locally: `scripts/check` via the pre-push hook; the GitHub workflows are manual-only (no Actions minutes)
- [x] Test-data corpus: `testdata/shp` (committed, `scripts/make-testdata.sh`) + `testdata/large` (`scripts/fetch-testdata.sh`)
- [x] `unigis-testkit`: oracle tests comparing our reads with `ogrinfo` (JSON attributes + ISO WKT geometry)

## Phase 1 — Shapefile (complete spec)
- [x] Esri-layout geometry model in `unigis-core` (flat coords + part offsets, Z/M, MultiPatch part types)
- [x] `.shp/.shx` all 14 shape types incl. Z/M and MultiPatch; empty points; missing/corrupt `.shx` recovery
- [x] `.dbf` (dBASE III/IV, field types C/N/F/L/D/M), `.cpg` / LDID encodings
- [x] `.prj` WKT1 passthrough
- [x] Writer: ArcGIS field-name rules, null substitution, clockwise shells, 2 GB limit
- [x] `ug describe` (arcpy.Describe) and `ug copy-features` (Copy Features)
- [x] Field mapping, null handling and value formatting verified against ArcGIS Pro 3.7
      through the ArcGIS bridge; `.dbf` output is byte-identical to ArcGIS's for the same input
- [x] `.sbn/.sbx` spatial index: written with every shapefile and by `ug add-spatial-index`,
      byte-identical on most references; rare layout gaps remain, with exact record/box
      and tree checks for the GARS boundary cases (docs/formats/shapefile-arcgis.md)
- [ ] `.shp.xml` metadata
- [x] DOS and Mac code pages in `.dbf` by language driver ID (437, 737, 850, 852, 857,
      860, 861, 863, 865, 866, 10000, 10007, 10029; GDAL's LDID table, as ArcGIS reads
      them); memo fields read as GDAL does (block number) — ArcGIS cannot open such files

## Phase 2 — Geometry engine
Overlay is delegated to `geo` (i_overlay); `unigis-geometry` adds Esri semantics and
`unigis-tools` the ArcGIS tools. Each tool is checked against ArcGIS Pro output stored
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
- [x] Buffer: geodesic for geographic data (`BufferDistance::Geodesic`, `unigis_geometry::buffer_geodesic`; `ug buffer
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
        `WITHIN_A_DISTANCE_GEODESIC` on longitude/latitude data (`unigis-geometry::nearest_geodesic`; metres on WGS 84,
        azimuth angles): equal to ArcGIS between points and for polygons; ArcGIS finds the closest point on a plain line
        only approximately, so line cases differ by up to 0.08 % in distance (`tests/near_geodesic.rs`, 54 cases). Segments
        are straight in longitude/latitude, so pairs across the antimeridian are not handled. The app's Near, Generate
        Near Table and Spatial Join use it (Select By Location distances still use the UTM zone).
- [x] Select Layer By Location (all 13 planar overlap types, search distance, invert;
      `ug select-by-location` writes the selection) and Append (TEST/NO_TEST, fields by
      name, other geometry types skipped with WARNING 000594) — match ArcGIS
- [x] Split (one shapefile per text value, named after it; ERROR 003911 for non-text
      fields) and Near with several near feature classes (`NEAR_FC`) — match ArcGIS
- [x] Spatial Join merge rules (First, Last, Join, Sum, Mean, Median, Mode, Min, Max,
      StdDev, Count, Range; `ug spatial-join --merge-rule FIELD=RULE[:DELIM]`): output types
      kept (integers rounded), numeric rules give text fields null, Count writes text, Mode
      ties go to the smallest value, sample standard deviation; Join text too long for its
      field empties that field and the rest of the row (WARNING 001156) — match ArcGIS
- [x] Spatial Join with a full field mapping string (`ug spatial-join --field-mapping`): output
      names, types (Long/Short/Float/Double/Text), lengths, text start/end positions, merge rules
      and several source fields per output (first non-listed-null source wins under First),
      values converted as ArcGIS does (doubles round to integers, text that is not a number
      reads as 0, numbers become text, text cut to the length) — matches ArcGIS; visible/alias
      are ignored for shapefile output as in ArcGIS
- [x] Merge with a field mapping (`ug merge ... --field-mapping`): output fields as mapped, each
      input contributing its own source field (found by table name), values converted to the
      output type — matches ArcGIS
- [x] Append with a field mapping (`--field-mapping`, NO_TEST): target fields take the first source
      belonging to the appended input, converted to the target's type — matches ArcGIS
- [x] Feature Class To Feature Class (`ug feature-class-to-feature-class`, where clause and field
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
- [x] Symmetrical Difference (`ug symmetrical-difference`): Union's one-input-only rows in its
      order, minus slivers narrower than the XY tolerance and vertices within it of their
      neighbours' line (spikes included) — matches ArcGIS. Union itself still keeps such slivers
- [x] Update (`ug update`, BORDERS): Erase's rows followed by the update polygons carrying the
      same-named fields — matches ArcGIS; NO_BORDERS not implemented
- [x] Sort (`ug sort in out "ZONE DESCENDING;VALUE ASCENDING"`): field keys (numbers by value, text by
      bytes, ties in input order) and `Shape` with UR/UL/LR/LL (the envelope corner's y then x),
      `ORIG_FID` added — matches ArcGIS on 12 cases
- [x] Delete Identical (`ug delete-identical`, in place or `--output`): first of each group kept in
      input order, FIDs closed up, "N duplicate(s) from M group(s) deleted." — matches ArcGIS
- [x] Add Field (TEXT/SHORT/LONG/FLOAT/DOUBLE/DATE with precision, scale, length; names fixed up with
      WARNING 000304; an existing name warns, ERROR 000012 for another type) and Delete Field
      (DELETE_FIELDS/KEEP_FIELDS; the last field stays, ERROR 002557) for shapefiles, in place or
      `--output` — match ArcGIS on 15 cases; Alter Field, Calculate Field still to do
- [x] Calculate Field (`ug calculate-field`, PYTHON3): a Python-subset interpreter (`pycalc`:
      arithmetic with Python semantics, comparisons, conditional expressions, str methods and
      slicing, `%` / `format` / f-string formatting, `math`, common builtins, a code block with
      `def`/`if`/`for`/`while`) with `!FIELD!` values; integers round half to even into integer
      fields, numbers become text, a failing or None row is ERROR 160145 for a shapefile; the
      field is created from `--field-type` if missing — matches ArcGIS on 28 expressions.
      Not covered: Arcade/SQL expressions, comprehensions, dictionaries, exceptions, imports
      besides `math`, date arithmetic
- [x] Planar graph tools (`unigis-tools::planar`; `ug polygon-to-line`, `ug feature-to-polygon`,
      `ug feature-to-line`, also on the registry): Polygon To Line IDENTIFY_NEIGHBORS (the default),
      Feature To Polygon (label features, ATTRIBUTES/NO_ATTRIBUTES, several inputs) and Feature To
      Line (several inputs, NO_ATTRIBUTES) match Pro 3.7 vertex for vertex on 22 reference runs and
      ~40 probes (`scripts/research/planar/`). Rules: coordinates within 2√2 × XY tolerance merge
      at their mean; rows ordered by top-right vertex (y, then x) and then the direction leaving it
      counterclockwise from west; P2L splits at nodes and ring starts, runs with the lowest-FID
      bounding polygon on its right, one row per (left, right) pair including polygons it lies in;
      FTP outer rings start after the top-right vertex, holes placed by an upward monotone-partition
      sweep and a walk along its diagonals; FTL keeps the first polygon's (else line's) direction
      on shared stretches, one row per combination of features of each input. Gaps: hole order and
      start with several holes side by side, an extra split ArcGIS sometimes makes in Feature To
      Line polygon rings (`docs/KNOWN_GAPS.md`)
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
- [x] Oracle: 137 of 145 test tables match GDAL; `ug describe` / `ug copy-features` from `.gdb`
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
      `ug` tool outputs now go into an existing `.gdb`. Checked in ArcGIS: it lists, describes and
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
      compacts and edits further, and `ug delete-rows/delete-features/truncate-table/add-field/calculate-field/
      delete-field` give the same fields, rows, indexes and next OBJECTIDs as the same ArcGIS tools
- [x] `rewrite_table`: replace the fields and rows of an existing table keeping its file number, geometry
      definition, chosen OBJECTIDs and surviving attribute indexes (the primitive for Add/Delete Field
      and in-place tools on `.gdb` data)
- [x] Editing in place from tools on `.gdb` data (`ug` in-place tools such as Add/Delete Field, Calculate
      Field, Append, Near, Delete Identical go through `rewrite_table`; OBJECTIDs the tool carries through
      are kept; `tests/gdb_in_place.rs`, `tests/gdb_edit_rows.rs`). Add/Delete/Calculate Field checked against
      ArcGIS; the other in-place tools not yet
- [x] Delete and Rename of tables, feature classes, relationship classes, feature datasets and domains
      (`delete_item`, `rename_item`, `delete_domain`, `rename_domain`; `ug delete/rename/delete-domain/
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
      coordinate system as ArcGIS writes them; classes take the dataset's XY domain; `ug create-feature-dataset`,
      outputs to `city.gdb/Ds/name`); ArcGIS lists, reads, edits and deletes in them. `ug` projects an output to
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
      `ug set-subtype-field` ... `ug assign-default-to-field`): definition XML, domain links and the table's own
      defaults equal ArcGIS's after 12 operations (`testdata/gdb-reference/subtype_ops`); ArcGIS applies them
- [x] Written geodatabases open in ArcGIS Pro 3.7: Describe, counts, domains, Check Geometry
      (no problems beyond those already in the source data), Copy Features out of them
- [x] Projected coordinate systems: ArcGIS's XY domain (origin, resolution, tolerance) for every system PROJ
      knows (7,135 by Esri name), collected from ArcGIS through the bridge (`scripts/make-xy-domains.py`,
      `xy_domains.tsv.gz`); used by file geodatabase and GeoPackage output. The origin comes from ArcGIS's own
      projection engine, so it is a table, not a formula; a system ArcGIS does not know keeps the fixed origin

## Phase 4 — CRS
- [x] PROJ binding (`unigis-proj` on the `proj` crate, system PROJ >= 9.6.2 or built from
      source): CRSs from factory codes (EPSG, then Esri), `.prj` files or WKT; Esri WKT out
- [x] Project tool (`ug project in out 4326`): vertices transformed by PROJ, Z/M kept — matches
      ArcGIS to ~1e-13° / sub-mm on WGS 84 ↔ UTM / Web Mercator; output `.prj` is ArcGIS's
      regenerated WKT, which every tool now writes (ArcGIS does not copy the input's text)
- [x] Project `transform_method`: ArcGIS's transformation names (`A + B` chains, either direction)
      found through PROJ's EPSG/Esri aliases and applied between the geographic systems
      (`ug project --transform-method`); ED50 ↔ WGS 84 / ETRS89 Helmert methods match ArcGIS on
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
  - [x] Stored statistics in PAM or TIFF GDAL_METADATA as ArcGIS uses them (count,
        min/max/mean/sample stddev; `%.14g` numbers). Copy Raster preserves cached
        fields; other outputs compute new caches. Both stores and PAM placement
        are compared, including120 embedded references. Basin writes count0;
        Expand DISTANCE includes its right/bottom workspace. See
        `docs/formats/raster-statistics.md`.
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
  - [x] Cell size/snap/extent/mask environments (`unigis-raster::env`, matched cell-for-cell to ArcGIS on 23 runs in
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
        cases (`ug kernel-density|point-density|line-density`). Point Density's grid starts
        (w + h) / 798 before the points' corner. Not done: barriers, GEODESIC method (planar
        only), map units other than meters, the app tools and arcpy entries
  - [x] Extract Values to Points (`ug extract-values-to-points`; plain and INTERPOLATE on integer, float
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
        `unigis-raster::interpolate`; `ug idw`, the app's IDW tool, `arcpy.sa.Idw` with
        `RadiusVariable`/`RadiusFixed`): grid cell centers start at the points' lower-left corner
        (`round(w / cell) + 1` cells; a snap raster snaps the points' own extent outward instead);
        default cell = shorter side / 250; variable radius = the n nearest points but never beyond
        the output grid's width + height; fixed radius widens a cell size at a time until
        `min_points` are inside; default fixed distance 5 cells; ~1.5k cells over 8 searches,
        3 environments and 15 reach cases match Pro 3.7 (`testdata/arcgis-reference/idw.json`,
        probes in `scripts/research/idw/`). Barriers not done. Next in this family: Natural
        Neighbor, Spline, Kriging, Trend, Topo to Raster
  - [x] Overlay and local tools over several rasters (`unigis-raster::overlay`; `ug cell-statistics`,
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
        Combine/Weighted Overlay attribute tables, `ug weighted-overlay`
  - [x] Construction tools on the shared registry (`unigis-tools::construct`): Densify, Generate
        Points Along Lines, Create Fishnet and Polygon To Line (IGNORE_NEIGHBORS), 24 runs matching
        Pro 3.7 (`testdata/arcgis-reference/construct.json`, probes in `scripts/research/construct/`):
        a segment longer than the distance is cut into floor(length / distance) + 1 equal parts (one
        no longer than it stays whole); points along lines walk the parts end to end, strictly
        before the total length, END_POINTS adding the first and last vertex; the fishnet's x-axis
        is the y-axis turned a quarter clockwise, polylines list the rows' lines then the columns'.
        Not done: fishnet labels, Densify's ANGLE and
        OFFSET methods and curves, chainage fields, Multiple Ring Buffer and Create Thiessen
        Polygons (probed: `scripts/research/construct/probe1.py`)
  - [x] Table tools on the shared registry (`unigis-tools::tables`): Copy Rows, Table To Table
        (where clause), Join Field (all or named fields, `_1` renaming, first match, nulls where
        unmatched; features or tables), Add Fields and Get Count matching Pro 3.7 on 9 runs
        (`testdata/arcgis-reference/tables.json`, `crates/unigis-cli/tests/tables.rs`). The
        registry now has table results (`Outcome::Table`). Not done: Delete Rows and Alter Field (they
        edit their input; shapefiles cannot rename fields), field mappings, GDB table outputs from
        `ug run`
  - [x] Data Management breadth tools on the shared registry (`unigis-tools::dm_geometry`,
        `dm_fields`, `dm_tools`; run with `ug run <id> input=... output=...`), each matching Pro 3.7
        on 3-9 runs in `manifest.json` (cases `env_*`, `slv_*`, `sap_*`, `epp_*`, `xy2l_*`, `cfs_*`,
        `std_*`, `tf_*`, `rf_*`, `transpose_*`, `pivot_*`; inputs in `testdata/dm`):
        Feature Envelope To Polygon, Split Line At Vertices, Split Line At Point, Eliminate Polygon
        Part, XY To Line, Calculate Fields, Standardize Field, Transform Field, Reclassify Field,
        Transpose Fields and Pivot Table, then Bearing Distance To Line (geodesic, short lines),
        Create Feature Class, Create Table, Define Projection, Copy, Calculate End Time, Unsplit Line, Alter Field and Alter Fields (file geodatabase tables, in place; `ug alter-field`, `ug alter-fields`), the domain tools Create Domain, Add/Delete Coded Value, Alter Coded Value Description, Set Value For Range Domain, Sort Coded Value Domain, Alter Domain, Domain To Table and Table To Domain (file geodatabase; 42 operations compared with ArcGIS state in `crates/unigis-filegdb/tests/domain_ops.rs`) and Dice (lines exact, polygons with small cells
        partly: see KNOWN_GAPS)
        (unjoined lines first, then joined ones; multipart and reversed pieces unchecked). Not done: Split Line At Point without a search radius,
        XY To Line / Bearing Distance To Line geodesics for long lines, Box-Cox by maximum
        likelihood, Reclassify Field GEOMETRIC_INTERVAL / NATURAL_BREAKS / STANDARD_DEVIATION, Add Incrementing ID Field (ArcGIS accepts it only on
        enterprise geodatabases), Subdivide Polygon, Generate Rectangles/Transects Along Lines.
        Known differences: the order of Split Line At Point's pieces on one curved road, and
        MULTIPART envelopes of nested parts (our shapefile writer turns the inner ring into a hole)
  - [x] Spatial Analyst math functions (`unigis-raster::mathfn`; `ug math-fn`, 45 app tools,
        `arcpy.sa.Abs` ... `BitwiseRightShift`): Abs, Exp, Exp2, Exp10, Ln, Log2, Log10, Square Root,
        Square, Negate, the trigonometric and hyperbolic functions and their inverses, Round Down/Up,
        Boolean Not, Bitwise Not, Power, Mod, ATan2, the six relational and three Boolean operators,
        Diff, Over and the bitwise operators matching Pro 3.7 on 125 runs, values, output types and
        NoData (`testdata/arcgis-reference/mathfn.json`): float results are NoData outside the
        domain or beyond a 32-bit float; integer results are typed for the range the statistics
        allow; Mod is fmod; shifts are 32-bit and wrap. Not done: Plus/Minus/Times/Divide type rules
        beyond the earlier ones (see `local::math_type`), Negate/Abs on constants
  - [x] Aggregate, Block Statistics, Slice (EQUAL_INTERVAL), Pick and Region Group (FOUR, WITHIN)
        (`unigis-raster::blockops`; app tools and `arcpy.sa`) matching Pro 3.7 on 82 runs, grids,
        types, NoData and values (`testdata/arcgis-reference/blockops.json`): Aggregate counts blocks
        from the lower-left corner, Block Statistics from the upper-left; an integer raster's range counts
        one wider for Slice. Not done: Slice's other methods (equal area differs on a cell, natural
        breaks), `ug` commands for Slice and Pick
  - [x] `ug aggregate`: SUM/MAXIMUM/MEAN/MEDIAN/MINIMUM, EXPAND/TRUNCATE and
        DATA/NODATA over single-band GeoTIFFs. CLI output grids, pixel types,
        NoData and every cell match all 40 saved ArcGIS Aggregate probes;
        omitted options reproduce SUM/EXPAND/DATA. Multiband input is refused
        pending verified support (`crates/unigis-cli/tests/aggregate.rs`).
  - [x] Block Statistics follow-up: 74 GeoTIFF comparisons against Pro 3.7 cover
        integer pixel types/NoData, negative integer storage, lower median,
        multiband output, circular/annular masks, DATA/NODATA and MAP rounding.
        App and `ug block-statistics` accept CELL/MAP neighborhoods and reject
        float counting statistics. All 75 probes also run through the CLI harness.
        App conversion retains every in-memory raster band; RGB Block Statistics
        is compared with ArcGIS through both pathless and non-TIFF inputs.
        Probe details and remaining limits: `scripts/research/blockstats/README.md`;
        regression tests: `crates/unigis-raster/tests/blockops.rs`.
  - [x] Expand, Shrink (MORPHOLOGICAL and DISTANCE), Majority Filter (FOUR/EIGHT, MAJORITY/HALF) and
        Region Group (FOUR/EIGHT, WITHIN/CROSS, excluded value) (`unigis-raster::generalize`; `gf
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
- [~] Tool registry: the app's `ToolSpec` registry (`app/crates/unigis`) holds ~65 ArcGIS-verified tools with typed
      parameters (layer, field, number, choice, text, bool), required/default checking, a hostile-parameter test over
      every tool, and geoprocessing history/undo from the RustGIS engine. Not yet: progress and cancellation for long
      runs, environments (cell size, extent, mask, snap) as first-class settings
- [~] Port tools by toolbox: Analysis (Clip, Erase, Intersect, Union, Identity, Update, Symmetrical Difference, Buffer, Near,
      Spatial Join, Select, Split, Frequency, Statistics), Data Management (fields, features, geodatabase schema tools via
      `ug`), Spatial Analyst (surface, hydrology, focal, local, zonal, reclass, distance, extraction) done; the rest of the
      2,157-tool inventory is not
- [x] Python bindings: the app's PyO3 package and MCP server dispatch through the registry (`gp.run` with
      `unigis:<tool>`), and `rustgis.arcpy` gives it ArcGIS's shape (`arcpy.analysis.Buffer("Cities", "out", "100 Kilometers")`,
      `management`, `conversion`, `sa`; inputs as layer names or paths, outputs written to paths; 10 tests)

## Phase 7 — Databases & other formats
- [x] Features To JSON: Esri JSON and GeoJSON written byte for byte as ArcGIS writes them
      (`unigis-json`: `%.17g` coordinates, field-scale-dependent number forms, formatted
      header with a one-line features array, wkid/latestWkid through PROJ's identify)
- [x] JSON To Features: Esri JSON (field types kept, the OID becomes an empty field,
      Shape_Length/Shape_Area added at 0 as ArcGIS's geodatabase detour leaves them) and GeoJSON
      (one geometry type, numbers as doubles, text(254), always 3D) — match ArcGIS
- [x] CSV (`unigis-csv`): tables read with ArcGIS's type inference (Long / Double /
      Text(254), usable by every tool), Export Table to `.csv` byte-identical to ArcGIS's (BOM,
      CRLF, `%.15f` doubles, RFC 4180 quoting), XY Table To Point (Z field, coordinate system)
- [ ] GeoPackage / SpatiaLite, PostGIS, KML, GML, DXF, E00, ERDAS IMG, JPEG2000
  - [x] Own read-only SQLite reader (`unigis-gpkg::sqlite`): header, table b-trees, overflow
        pages, records with type affinity, CREATE TABLE columns and rowid aliases; checked row
        for row against Python's sqlite3 at page sizes 512, 4096 and 65536
  - [x] GeoPackage feature and attribute tables read as ArcGIS reads them (`main.` names, 64-bit
        ObjectID, SQLite types → ArcGIS field types, Esri ring orientation keeping start
        vertices, empty points), usable as input to every `ug` tool; tools that ArcGIS refuses
        to run from 64-bit ObjectIDs into shapefiles fail the same way
        (`docs/formats/geopackage.md`)
  - [x] Shapefile output: Date Only fields fail with ERROR 000308; empty points written as
        −DBL_MAX as ArcGIS does
  - [x] Own SQLite writer (`unigis-gpkg::sqlite_writer`): bulk-loaded table and index
        b-trees, overflow pages, automatic indexes, schema beyond page 1, lock-byte page; passes
        `PRAGMA integrity_check` at page sizes 512–65536 and SQLite keeps editing the files;
        2 million rows in 0.18 s (≈ 8× SQLite's own bulk insert), with an index 0.53 s (≈ 3×)
  - [x] GeoPackage writing as ArcGIS does (`ug <tool> ... out.gpkg/name`): ArcGIS's schema for a
        new file, feature tables with its column definitions and checks, OBJECTID from 1,
        little-endian blobs without envelope (MULTIPOLYGON/MULTILINESTRING, OGC ring order),
        coordinates snapped to ArcGIS's XY domain, Esri WKT and WKT2 spatial reference rows,
        R*Tree spatial index bulk-loaded (passes `rtreecheck`) with ArcGIS's triggers; equal
        row for row to ArcGIS's Copy Features output; ArcGIS reads, queries and edits the
        files. Adding a table rewrites the file. 1 million points in 0.5 s (GDAL: 1.3 s)
  - [x] ArcGIS XY domains: geographic, Mercator variants, Plate Carrée exact, Transverse
        Mercator's y origin exact (x approximate)
  - [x] Export Table into a GeoPackage (`ug export-table in out.gpkg/name`): attribute tables
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
      registered in its `ToolSpec` registry (`app/crates/unigis`, vector and raster tools); next: more tools
- [ ] Layout view + PDF/PNG/TIFF export; 3D scene view

## Phase 9 — Advanced
- [ ] Network analysis, geocoding, spatial statistics, geostatistics, image classification, point clouds (LAS/LAZ)
  - [ ] Spatial Statistics (0 of 63 at the start of this item)
    - [x] Measuring Geographic Distributions (`unigis-tools::spatstat`; `ug mean-center`,
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
          (`testdata/arcgis-reference/spatstat_patterns.json`, `crates/unigis-cli/tests/spatstat_patterns.rs`).
          Tools that give values rather than features return a `registry::Report` (`ug run` prints
          it, the app shows a table and messages, arcpy `getOutput(i)`); ArcGIS's p-values are a
          normal-CDF approximation up to 2e-8 from the exact ones we use. Not done: polygon
          contiguity conceptualizations, spatial weights files, distances under 1 (treated as 1
          for inverse distance, unchecked), FDR checked only where nothing is significant
    - [x] Spatial weights (`unigis-tools::swm`, `docs/formats/swm.md`): the `.swm` file format
          (read/write, 32- and 64-bit IDs), Generate Spatial Weights Matrix (inverse/fixed distance
          with exponent, threshold and least neighbors, K nearest, Delaunay = Voronoi clipped to
          the hull, contiguity edges/corners with orders, space-time window, convert table;
          bisquare/Gaussian/triangular/Epanechnikov kernels, field and border-length weights,
          focal feature, row standardization, header and summary messages) and Convert Spatial
          Weights Matrix to Table matched to Pro 3.7 on 70 files, most byte for byte (neighbor
          order: ArcGIS's k-d tree built with MSVC `nth_element`; sums: NumPy pairwise).
          Spatial Autocorrelation, Hot Spot Analysis and the new Cluster and Outlier Analysis
          (Anselin Local Moran's I) take weights files and polygon contiguity; an empty distance
          band is the default threshold, 0 relates every pair (`crates/unigis-cli/tests/spatstat_weights.rs`).
          Open: KNN tie order on grids, set order in Delaunay/contiguity rows (KNOWN_GAPS)
    - [x] Multi-Distance Spatial Cluster Analysis (Ripley's L(d); weights, beginning distance and
          increment, minimum enclosing rectangle or study area polygon, Ripley's edge correction
          up to the corner case) matched to Pro 3.7 (`crates/unigis-cli/tests/spatstat_ripley.rs`).
          Not done: simulated outer boundary and reduced analysis area corrections
    - [x] Ordinary Least Squares (`unigis-tools::regression`): coefficients with standard and
          White HC0 robust errors, standardized coefficients, the diagnostic table (AIC/AICc, R2,
          F, Wald, Koenker BP, Jarque-Bera, sigma2 with ArcGIS's definitions), Estimated /
          Residual / StdResid per feature and Spatial Autocorrelation of the residuals matched to
          Pro 3.7 (`crates/unigis-cli/tests/spatstat_ols.rs`); the report file is not written
    - [x] Collect Events (exact-location counts in x, y order, `Id` for shapefiles) matched to
          Pro 3.7 (`crates/unigis-cli/tests/spatstat_events.rs`)
    - [ ] Next: GWR, Incremental Spatial Autocorrelation, Calculate Distance Band from Neighbor
          Count, Generate Network Spatial Weights

## Shared agent workflow

- [x] Shared Claude Code / Codex startup and checkpoint instructions (`AGENTS.md`,
      referenced by `CLAUDE.md`), with current task state in [`HANDOFF.md`](HANDOFF.md).
      Sequential sessions use the same checkout; agents checkpoint at milestones
      and inspect the working tree when resuming.
- [x] Local coordinator (`scripts/agents`): shared terminal prompt, per-provider
      session resumption, run logs and handoff snapshots, repository lock, and a
      single fallback to the other CLI on a recognized usage-limit failure.
      Native CLI escape for interactive approvals; offline integration coverage.

## Current desktop integration (2026-10-23)

- [x] Startup defaults to a new map with OpenStreetMap. The official Basemaps plugin
      installs in the background on first launch, with a pinned package hash and
      persistent desktop/browser storage. Existing sample preferences, projection/
      extent settings and saved projects are preserved; later plugin removal is respected.
- [x] Dockable panes: left/right/bottom groups, tab dragging and drop previews,
      auto-hide, scrollable tab headers, and saved layout preferences. Reopening
      panes preserves tab order; pointer-drag and preference round-trip tests pass.
      The attribute table remains a separate bottom view; floating windows and
      arbitrary split groups are not implemented.
- [x] Unified tool catalog: one ArcGIS toolbox/toolset tree with verified tool
      replacements, legacy ids and parameter translation. Aliases retain old
      defaults; selections survive renamed layers. Registry and integration tests pass.
- [x] UniGIS display branding: desktop, browser/PWA, assistant, translations,
      dialogs, and current documentation. Existing crate names, tool ids, Python
      exception names, file extensions and storage paths remain compatible.

Validation (2026-10-24): full app `cargo xtask ci` passes (716 tests passed,
10 ignored, zero failures), including local-server tests with full host access;
no sandbox skip/filter flags. Formatting, full workspace/all-targets clippy with
`-D warnings`, layer checks and the separate wasm web check pass. Root workspace
validation passes 173 tests (5 ignored). GDAL provider checks use a writable
cache; no runtime GDAL behavior was changed. Commands/results are in `HANDOFF.md`.

- Zonal Histogram and Tabulate Area (Spatial Analyst, `ug zonal-histogram`, `ug tabulate-area`): 12 ArcGIS cases; Zonal Geometry still to do (see KNOWN_GAPS).
- Cost Distance, Cost Back Link and Cost Allocation (`ug cost-distance`, `cost-back-link`, `cost-allocation`): 14 ArcGIS cases, distances exact; see KNOWN_GAPS for ties.
- Corridor, Filter (LOW/HIGH, `ug filter`) and Nibble (`ug nibble`; ties take the smallest value): 15 ArcGIS cases.
- Add/Remove Attribute Index for file geodatabase tables (`ug add-index`, `remove-index`; single field, file for file equal to ArcGIS on 20 operations). Spatial index add/remove and multi-field indexes are not done.
- Reclass by Table (`ug reclass-by-table`; integer output field required, float fractions are truncated): 7 ArcGIS cases. `ug` now reads a stand-alone .dbf as a table.
- Raster registry (`ug run <id>`; `RUN` in the reference script drives the test): Nibble and Filter moved to it; added Slice (equal interval/area; ~10 boundary cells differ), Extract by Attributes/Rectangle/Circle, Lookup, Sink, Pick, Flip, Mirror, Shift and Clip (rectangle) with 40+ ArcGIS cases.
- Pairwise Buffer/Dissolve/Intersect/Clip/Erase (aliases of the classic tools, `crates/unigis-cli/src/aliases.rs`), Multiple Ring Buffer, Generate Points Along Lines chainage fields, Copy Rows field widths: ArcGIS cases `pw*`, `mrb_*`, `gpal_*`, `cr_*`.

- [x] Graphic Buffer (`ug run graphic_buffer`): constant and per-feature numeric/text planar distances, square/butt/round caps and miter/bevel/round joins; 48 ArcGIS cases cover points, tight bends, polygon holes, negative/zero polygon distances, reserved-field replacement and miter limits 1/1.25/2/10. Over-limit corners bevel as ArcGIS does; numeric nulls/field zeros skip, field lookup ignores case, imperial unit definitions match ArcGIS. Geographic and malformed/null text inputs are explicitly refused. Limits: `scripts/research/graphic_buffer/README.md`.

- [x] Development dashboard: implementation-aware reference coverage, eight offline regression tests, latest known-gaps section, and cached CI results with a visible fetch timestamp that survives post-commit rebuilds.

- [x] Composite Bands (`ug run composite_bands`): concatenates every input band in order on the first grid, nearest-neighbor sampling, verified pixel-type promotion and ArcGIS NoData order behavior; 125 saved ArcGIS cases include every pairing of eight pixel types, per-source padding and mixed-type NoData defaults. Typed fixture creation verifies stored UInt32/Float64 boundary values by read-back. Generic raster comparisons and statistics writing now cover every band. Limits: `scripts/research/composite/README.md`.

- [x] CLI tool inventory exits cleanly when a pipe consumer closes stdout early; ordinary listing still exposes every registered tool. Deterministic closed-output regression test covers the former panic.

- [x] ASCII To Raster / Float To Raster (`ug run ascii_to_raster|float_to_raster`): native header/value decoding, S32 truncation/F32 output, system NoData replacement, center/corner origins and both binary byte orders. 35 ArcGIS success/error cases, exact converted pixels, and a read-only failure guard; projection sidecars ignored as in the legacy ArcGIS tools. Limits: `scripts/research/grid_import/README.md`.

- [x] Raster Define Projection (`ug run define_projection`): EPSG assignment/replacement without changing cells or grid, five ArcGIS cases and multiband/in-place/failure safety tests. GeoTIFF geographic keys now recognize GDA2020 and other geographic EPSG identifiers outside 4000..4999; raw-key and GDAL format checks cover both geographic and projected systems. Limits: `scripts/research/raster_crs/README.md`.

- [x] Copy Raster retains every band and applies pixel conversion to each; 24 additional ArcGIS cases cover RGB layouts, all eight source pixel types, NoData defaults outside integer storage ranges, and first/later-band NoData statistics. Exact all-band cell comparisons prevent silent band loss. Research: `scripts/research/copy_raster/README.md`.

- [x] Copy Raster downcasts: 16 additional ArcGIS cases cover all eight types to F32/I16. Large float NoData metadata survives integer output, and statistics exclude the rounded F32 marker correctly. A GeoTIFF/GDAL regression checks tags beyond the i64 range.

- [x] Equal To / Greater Than / Less Than Frequency (`ug run *_frequency`): exact source-precision comparisons, duplicate counts, both band modes, value-band broadcast, missing/extra comparison bands, ArcGIS count types/NoData and empty-band statistics.66 saved cases plus CLI refusal preservation; empty Rank/Popularity lists now return a parameter error instead of panicking. Limits: `scripts/research/frequency/README.md`.

- [x] Abs / Negate / Square preserve uncached Float64 cells and the F64 NoData marker; cached Float64 inputs follow ArcGIS narrowing to Float32. Twelve exact ArcGIS cases cover tagged/untagged and cached/uncached doubles. Integer paths have separate cached/uncached reference corpora below.

- [x] Uncached integer Abs/Negate/Square: 39 additional ArcGIS cases cover six integer types with/without NoData plus exact signed/unsigned32 boundaries. Negate uses observed widths and signed32 wrapping; Square preserves signedness, widens appropriately and marks integer overflow NoData. All48 original typed probes now match; cached integer planning is covered by the subsequent 81-case milestone below.

- [x] Explicit GP encoding for integer NoData outside sample storage: zero physical cells, original tag and logical statistics preserved. GeoTIFF API retains default conversion; CLI opts in. Frequency255-with-NoData now works with three additional ArcGIS cases; native/GDAL tests cover U8/I16/I32, tiled/striped and BigTIFF layouts.

- [x] Cached integer Abs/Negate/Square planning: 81 ArcGIS cases/27 snapshots with original statistics sidecars, conservative ranges, narrow packed/signed types, signed/unsigned32 endpoint choices, wrapping/overflow and out-of-range NoData encoding. Uncached and Float64 paths retain separate references. Reproducer: `scripts/research/unary_types/README.md`.

- [x] Line Statistics (`ug run line_statistics`): clipped-length weighting, all eight statistics, integer/double output types, lower-value mode ties and empty-neighborhood semantics; 22 ArcGIS cases and invalid-parameter/allocation guards. Planar straight polylines with explicit cell size/radius; remaining scope in `scripts/research/line_statistics/README.md`.

- [x] Copy Raster statistics policy: 22 additional ArcGIS cases isolate cached/uncached inputs, size, band count, unused tags, type changes and equal/changed markers. Ordinary tagged cached copies reuse embedded statistics without PAM when type/marker stay unchanged; UInt32 system-marker cases are separately verified. Reproducer: `scripts/research/copy_raster/statistics/README.md`.

- [x] CLI GeoTIFF overwrite lifecycle: no-statistics outputs remove stale PAM metadata after a successful TIFF write. Replacement, existing native in-place behavior and invalid-parameter byte preservation have a dedicated regression; ArcGIS overwrite probe confirms clearing old statistics.

- [x] Copy Raster tagged/default NoData:48 additional exact cases cover UInt32 sign/precision boundaries, all eight tagged types, destination types, overrides and converted source sentinel payloads. Cached statistics are preserved in TIFF; other no-PAM GP outputs compute embedded statistics. Generic validation now checks both stores across120 previously unchecked embedded-cache references. Native/GDAL classic/BigTIFF and PAM-override tests cover the format API.

- [x] Copy Raster fractional conversion:44 additional exact ArcGIS cases cover ties-even NoData markers, midpoint-up integer pixels, signed storage limits, converted source payloads, unchanged cached markers and negative unsigned raw TIFF tags. Logical statistics masking stays separate from physical tags; native/GDAL classic/BigTIFF tests cover the opt-in writer override. Research: `scripts/research/copy_raster/fraction/README.md`.

- [x] Copy Raster partial caches:11 additional ArcGIS cases cover missing extrema/moments/count, stale complete caches and ancillary statistics fields. Missing either moment resets mean/stddev to0/-1; incomplete extrema trigger fresh PAM statistics. A CLI regression compares every stored field for all eight embedded-cache cases. Research: `scripts/research/copy_raster/cache/README.md`.

- [x] Complete multiband raster caches and floating unary bands:20 ArcGIS cases cover per-band extrema, default/custom domains, single-quoted XML, stale/partial caches, and F32/F64 Abs/Negate/Square. Shared metadata parsing requires cached extrema in every band; the three floating unary paths now preserve all bands. Native cache-planning regression covers11 snapshots. Research: `scripts/research/copy_raster/multicache/README.md`.

- [x] Recently committed raster tools validated by the full workspace gate: Diff/Over (14 ArcGIS cases), single-band Compute Change Raster DIFFERENCE/RELATIVE_DIFFERENCE (10), and basic Create Binary Mask (5). Remaining modes are recorded in KNOWN_GAPS.
- [x] Generate Tessellation square, diamond, hexagon and triangle:18 ArcGIS cases cover projected extents, cell geometry, ordering and GRID_ID fields. Transverse hexagons have subsequent coverage below; H3 shapes remain follow-up work; explicit size-unit conversion is covered below.

- [x] Generate Tessellation transverse hexagons:6 additional ArcGIS cases verify row staggering, extent coverage, IDs, row-major order and clockwise rings, including tiny and thin extents. Research: `scripts/research/tessellation/transverse/README.md`.

- [x] Generate Tessellation area units and safe refusals:31 ArcGIS cases cover18 areal units, projected-meter/US-foot coordinates, bare sizes, missing SR and all five supported shapes. Explicit sizes now convert instead of silently discarding suffixes; a CLI regression guards existing outputs against malformed/extreme parameters and overflow panics. Provided Esri Foot_US CRS definitions retain parameter precision and unit names during output export. Research: `scripts/research/tessellation/area_units/README.md`.

- [x] Uncached integer multiband Abs/Negate/Square:21 ArcGIS cases across six integer types and reversed I16 band order verify both bands, overflow, NoData, type/grid/CRS and statistics. Existing storage-dependent planners now run for every band. Cached integer multiband requires a separate range-model fix; no app API fields changed. Research: `scripts/research/unary_multiband/README.md`.

- [x] Unary multiband safety: registered unary tools explicitly refuse unsupported band/type/cache combinations before writing output. A shared support predicate keeps the dispatcher and refusal consistent; six CLI refusal cases verify byte-identical preservation of existing GIS files. Three additional ArcGIS probes establish the previous SquareRoot/Ln/Exp band-loss mismatch.

- [x] Floating multiband Square Root/Ln/Exp:12 ArcGIS cases cover cached/uncached F32/F64, invalid domains, exponential overflow and independent per-band missing cells. Both bands use the existing F32 math paths; unsupported combinations still fail before output. Research: `scripts/research/unary_multiband/floating/README.md`.

- [x] Cached integer multiband Abs/Negate/Square:36 ArcGIS cases verify all-band stored-range planning across six types, reversed order, positive127 promotion and GP stored-zero/covariance statistics for empty overflow bands (including one/three-band controls). The reader retains per-band metadata while exposing a union for type planning; invalid complete multiband extrema fail safely. Research: `scripts/research/unary_multiband/cached/README.md`.

- [x] Floating multiband Log2/Sin/Round Up:13 direct ArcGIS GP cases cover F32/F64 cached/uncached, invalid logarithm domains and independent missing cells. A32value precision fixture verifies F32 Log2 natural-log scaling, while F64 stays unchanged. Reuses validated floating sources and the per-band dispatcher; the guard still rejects remaining operators and malformed cached integer ranges. Research: `scripts/research/unary_multiband/more/README.md`.

- [x] Convert Coordinate Notation (DD/DDM/DMS/DD_NUMERIC and UTM_BANDS/UTM_ZONES/MGRS/USNG/GARS/GEOREF output from lon/lat; 13 `ccn_*` cases), Table To Ellipse (planar; adaptive complete-ellipse densification; 5 `ellipse_*` cases), Geodetic Densify (geodesic; 6 `gdd_*` cases), Encode Field (One-hot/One-cold; 4 `enc_*` cases) and Field Statistics To Table (numeric/text/date, optional group-by field and Output Statistics list; 17 `fst_*` cases incl. 4 refusals) match ArcGIS; gaps are in docs/KNOWN_GAPS.md and docs/HANDOFF.md.
- [x] CI gate runs locally (`scripts/check`, pre-push hook); GitHub workflows are manual-only because Actions minutes ran out.

- [x] Convert Coordinate Notation (DD_1/DD_2/DDM_1/DDM_2/DMS_1/DMS_2/DD_NUMERIC/UTM_BANDS/UTM_ZONES/MGRS/USNG/GARS/GEOREF from lon/lat), Table To Ellipse (planar), Geodetic Densify (geodesic), Encode Field (one-hot/one-cold) and Field Statistics To Table (numeric/text): reference cases match ArcGIS; gaps in KNOWN_GAPS.md and HANDOFF.md. Not done: DDM/DMS and grid outputs from projected-grid inputs, H3 output, geodesic ellipses.

- [x] Convert Coordinate Notation angular text inputs: DD/DDM/DMS single- and two-field
      formats, hemisphere overrides/axis order, packed forms and invalid-row retention.
      Twelve new ArcGIS reference cases compare numeric and DMS outputs, schema and NULL
      geometry; `input_format` defaults to DD_2 for existing callers. Grid input parsing
      and remaining grammar/projection limits: `scripts/research/coordinate_notation/README.md`.

- [x] Convert Coordinate Notation GARS/GEOREF to decimal degrees and GEOREF:
      cell/quadrant/keypad and minute precision variants, case/whitespace,
      minute overflow and invalid-row retention. Six ArcGIS reference cases
      cover 81 GARS and 87 GEOREF inputs. Unresolved output modes fail before
      writing; a CLI guard preserves existing output files. Findings/remaining
      grid-output boundary and angular carry rules:
      `scripts/research/coordinate_notation/grid/README.md`. UTM/MGRS/USNG inputs remain.

- [x] Convert Coordinate Notation grid-input DDM/DMS outputs: symmetric relative
      carry-to-60 policy matches 5,634 exact ArcGIS sweep/refine controls across
      both axes/hemispheres, plus 100 independent varied-cell precision controls.
      Twelve new cases cover four formats, both GARS/GEOREF inputs and a typed
      148-row DBF precision fixture. Default ORIG_OID retention follows explicit
      source identity metadata (CSV has synthetic row IDs only); DBF/shapefile/GDB
      controls match ArcGIS. GARS output boundaries have subsequent coverage below.
      Evidence and CSV auto-coordinate schema gap: `scripts/research/coordinate_notation/grid/carry.md`.

- [x] Convert Coordinate Notation GARS output from GARS/GEOREF inputs: positive
      decoder frame/origin ordering, degree/radian roundtrip and direct tick
      floors match 240 prior and 1,144 independent corner/precision controls.
      Four reference cases cover 1,312 rows; two reuse unchanged saved ArcGIS
      outputs. Existing 18 grid cases and 5,634 carry controls remain exact.
      The angular numeric GARS boundary path remains separate follow-up work.
      Evidence: `scripts/research/coordinate_notation/grid/gars-boundaries.md`.

- [x] Calculate Geometry Attributes (planar properties into existing fields, length/area units):
      5 in-place `cga_*` cases over polygons, lines and points match ArcGIS. Evidence:
      `scripts/research/calculate_geometry_attributes/README.md`.
- [x] Adjust 3D Z (reverse, constant or field shift, US-survey unit conversion): 6 in-place `a3z_*`
      cases over point, line, polygon and multipoint Z shapefiles match ArcGIS. Evidence:
      `scripts/research/adjust_3d_z/README.md`.
- [x] Excel To Table (`excel_to_table`): native .xls/.xlsx, first/selected sheet,
  rectangular .xlsx named range, header row/column-letter names and cell ranges;
  numeric/text/boolean schemas, NULLs, aliases and DBF dates.13 ArcGIS portable
  references plus8 FileGDB schema/value controls; format/other limits in
  `scripts/research/excel_to_table/README.md` and KNOWN_GAPS.
- [ ] Make Query Table: deferred; session query-view handles/key identity and SQL
  joins absent from Context/Outcome.3 public controls in
  `scripts/research/make_query_table/` confirm FeatureLayer/TableView semantics.
- [ ] Convert Personal Geodatabase: deferred; no MDB/ACCDB input parser or default
  mobile geodatabase writer. FILE_GDB/XML alternatives still need the input parser.
- [x] Add Global IDs (`add_global_ids`): in-place FileGDB class/table and simple
  feature-dataset expansion, batches in one workspace, UUID4 values, system field
  metadata/index and repeat stability. Eight public references plus rollback tests;
  evidence `scripts/research/add_global_ids/README.md`.
- [x] Subset Features (`subset_features`): percentage/absolute training size,
  optional complementary test output, feature/table schemas and input order,
  native seeded sampling and observed DBF NULL conversion. Coordinated new folder
  or same-FileGDB outputs;16 public partition references plus rollback/safety tests.
  Limits and public evidence: `scripts/research/subset_features/README.md`.
- [x] Create Spatial Sampling Locations (`create_spatial_sampling_locations`):
  polygon RANDOM, STRAT_POLY/STRAT_ID with equal/area/population allocations, spacing;
  SYSTEMATIC with five area-unit bin shapes and point/polygon inclusion modes.
 23 public references; random contracts and strict systematic comparisons. Raster,
  cluster, H3, count-based bin search and environments remain gaps. Research:
  `scripts/research/spatial_sampling/README.md`.
- [x] Create Random Points (`create_random_points`): planar extent/polygon/line
  sampling and point subsets, per-feature count/distance fields, spacing/saturation,
  multipoint chunks and CID/Id. Native reproducible seed; random coordinates differ
  from ArcGIS.16 public property-reference cases and distribution/safety tests.
  Scope/gaps: `scripts/research/create_random_points/README.md`.
- [x] Extract Data From Geodatabase (`extract_data_from_geodatabase`): single-source
  FileGDB standalone classes/tables, DATA/SCHEMA_ONLY, new/existing destination,
  default/all table rows, simple relationship expansion/related rows, domains,
  field aliases/defaults, OID gaps and single-field indexes. Unsupported output
  formats, schema reuse, geometry filters and advanced datasets refused. Public
  `edg_*` workspace references; scope/gaps in `scripts/research/extract_data/`.
- [x] Validate Join (message table: unindexed join fields, no-match and row-count rows;
      one-to-one match count; self-join refusal): 4 `vj_*` cases + 1 refusal match ArcGIS for
      shapefile/dBASE tables. Evidence: `scripts/research/validate_join/README.md`.
- [x] Convert Time Zone (139 ArcGIS/Windows zone names via CLDR -> IANA rules with chrono-tz,
      DST-adjusted or standard-time input/output): 4 `ctz_*` cases + a 72-instant unit test
      match ArcGIS. Evidence: `scripts/research/convert_time_zone/README.md`.
- [x] Convert Temporal Field (date <-> text/number with .NET-style formats, lenient parsing,
      WARNING 002125/ERROR 002126): 10 `ctf_*` in-place cases + 1 refusal match ArcGIS. Evidence:
      `scripts/research/convert_time_field/README.md`.
- [x] Batch Update Fields (definition-table columns; source copy with type conversion, blank
      type = source type, fixed-decimal rounding, scripts over output fields in dependency order;
      cycle/missing/type refusals): 3 `bfu_*` cases + 3 refusals match ArcGIS. Evidence:
      `scripts/research/batch_update_fields/README.md`.
- [x] Add Spatial Join (permanent fields: target fields, Join_Count, distance field, join
      fields; values of a one-to-one Spatial Join): 3 `asj_*` in-place cases match ArcGIS.
      Evidence: `scripts/research/add_spatial_join/README.md`.
- [x] Trim Line (DELETE_SHORT/KEEP_SHORT; cuts at crossings, touches, own parts and self
      crossings; per-part short features; input-geometry based): 8 `trim_*` cases on a 20-line
      fixture match ArcGIS. Evidence: `scripts/research/trim_line/README.md`.
- [x] Extend Line (FEATURE and EXTENSION; sequential OID order, strict length as for
      shapefile edits, potential-extension targets): 6 `ext_*` cases on a 28-line fixture match
      ArcGIS. Evidence: `scripts/research/extend_line/README.md`.
- [x] Convert Coordinate Notation UTM_BANDS/UTM_ZONES/MGRS/USNG input (Krüger inverse
      transverse Mercator and UPS; reduced-precision MGRS, band-driven row cycles, polar
      grids, antimeridian wrap) to DD_NUMERIC/DD_1/DD_2: 8 reference cases over 792 rows
      match ArcGIS. DDM/DMS/grid outputs refused: ArcGIS's inverse differs by up to
      ~3e-10 degrees. Evidence: `scripts/research/coordinate_notation/grid/projected-inputs.md`.

- [x] Assign Weights By Pairwise Comparison: 2–9 variables, default equal weights,
      ordered pair strings/fractions, ADD_MATRIX and WEIGHTS_ONLY. Six successful
      ArcGIS cases and four refusals; findings in scripts/research/pairwise_weights/.

- [x] Generate Origin-Destination Links: native planar implementation and 14
      saved ArcGIS cases (groups, integer destination limits, search/units,
      self/coincident links, centroids, empty output) match the ArcGIS controls.
      Geodesic and aggregated output remain gaps; findings in
      scripts/research/origin_destination/README.md.

- [x] Pairwise Polygon Neighbors: optional ObjectID/report fields, projected
      edge/node/area relationships, grouped hierarchy, BOTH_SIDES/NO_BOTH_SIDES,
      output units.14 ArcGIS cases (176 rows plus2 refusals) match native output;
      independent aligned-overlap and reversed-key controls cover Pairwise
      differences. Full local gate passed; findings in scripts/research/pairwise_neighbors/.

- [ ] Remove Overlap (multiple): deferred; default CENTER_LINE needs a medial-axis
      partition primitive absent from the native engine.18 controls saved in
      scripts/research/remove_overlap/; reason recorded in WORK_QUEUE_DEFERRED.txt.
- [x] Split By Attributes: folder shapefile/DBF partitions by text/32-bit integer
      fields, multiple keys, source metadata/order, safe unique naming and existing
      output preservation.11 ArcGIS cases; full local gate passed. Host workspace
      capability is explicit; other hosts and advanced types remain gaps.
      Findings: scripts/research/split_by_attributes/README.md.

- [ ] Pairwise Integrate: deferred; default cluster/collapse behavior is absent
      from native primitives.33 public controls, including default XY tolerance,
      saved in scripts/research/pairwise_integrate/README.md.

- [ ] Add Full-Text Index: deferred; requires unsupported SQLite/mobile/enterprise
      database backend. FileGDB control ERROR002764; scripts/research/full_text_index/.

- [x] Add GPS Metadata Fields: native implementation and14 public ArcGIS cases;
      default/offset schemas,4 simple geometry types, Z/M, existing fields,
      domains and idempotence. Native FileGDB/CLI and optional host capability;
      full local gate passed. Research: scripts/research/gps_metadata/README.md.

- [x] Delete Multiple: native preflight/deletion and13 ArcGIS reference cases
      match ArcGIS; full local gate passed. Local shapefile/DBF/TIFF families,
      regular files and FileGDB items/workspaces. Folder recycling and ambiguous
      kind selection remain gaps. Research: scripts/research/delete_multiple/README.md.

- [x] Disable COGO: native FileGDB metadata edit and shapefile no-op implemented;
      8 ArcGIS references, original dimension fields/values retained. Full local
      gate passed; layer styling and other hosts remain gaps. Research:
      scripts/research/disable_cogo/README.md.

- [x] Disable Editor Tracking: FileGDB classes/tables and simple feature datasets;
      default and four selective disable options preserve fields/values/time basis.
      13 ArcGIS references plus dataset/byte-preservation safety checks. Research:
      scripts/research/disable_editor_tracking/README.md.

- [x] Enable COGO: native FileGDB line classes, five dimension fields/required
      markers and canonical aliases; retains existing values and OID holes.
      11 ArcGIS references plus refusal/idempotence safety controls. Research:
      scripts/research/enable_cogo/README.md.

- [x] Enable Editor Tracking: simple FileGDB classes/tables, optional roles,
      ADD_FIELDS/NO_ADD_FIELDS, UTC/DATABASE_TIME and active-state preservation.
      14 public references and native field/read-only/value/no-op safety checks.
      Later native edit auto-stamping remains a gap. Research:
      scripts/research/enable_editor_tracking/README.md.

- [x] Migrate Date Field To High Precision: FileGDB tables/classes, one/multiple
      Date fields, metadata-only precision migration retaining values and aliases.
      12 public references and byte-preservation/refusal/idempotence safety checks.
      Research: scripts/research/migrate_date_precision/README.md.
