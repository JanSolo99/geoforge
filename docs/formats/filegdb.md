# File Geodatabase format notes

Working notes for `geoforge-filegdb`. Derived from Even Rouault's reverse-engineered
specification, https://github.com/rouault/dump_gdbtable/wiki/FGDB-Spec (CC-BY-SA 3.0),
and checked against GDAL's OpenFileGDB driver output. Clean-room: no Esri SDK code.

All integers little-endian. `varuint`: 7 bits per byte, msb = continuation.
`varint`: same, but bit 0x40 of the first byte is the sign (first byte carries 6 bits).

## Files
`aXXXXXXXX.gdbtable` (rows), `.gdbtablx` (row offsets), `.gdbindexes`, `.atx`
(attribute index), `.spx` (spatial index), `.freelist`. Hex number = FID of the table
in the system catalog `a00000001` (fields `Name`, `FileFormat`). v10: a1..a8 reserved
(a4 = GDB_Items with XML `Definition`, `Path` like `\FeatureDataset\Name`).

## .gdbtable header (40 bytes)
| off | v3 (32-bit OID) | v4 (64-bit OID, Pro 3.2) |
|---|---|---|
| 0 | i32 version = 3 | i32 version = 4 |
| 4 | i32 valid rows | i32 "has deleted" flag |
| 8 | i32 max row size | i32 max row size |
| 12 | i32 = 5 | i32 = 5 |
| 16 | 8 bytes unknown | i64 valid rows |
| 24 | i64 file size | i64 file size |
| 32 | i64 offset of field section | same |

## Field section
`i32 size, i32 version (3=9.x, 4=10.x, 6=Pro 3.2 ext), u32 layer flags, i16 nfields`.
Layer flags: `& 0xff` geometry type (0 none, 1 point, 2 multipoint, 3 polyline,
4 polygon, 9 multipatch); bit 8 = strings UTF-8 (else UTF-16); bit 30 = has M;
bit 31 = has Z.

Per field: `u8 nameLen (UTF-16 units), utf16 name, u8 aliasLen, utf16 alias, u8 type`.
Types: 0 i16, 1 i32, 2 f32, 3 f64, 4 string, 5 datetime, 6 objectid, 7 geometry,
8 binary, 9 raster, 10 GUID, 11 GlobalID, 12 XML, 13 i64, 14 DateOnly, 15 TimeOnly,
16 DateTimeWithOffset. Field flag bits: 0 nullable, 1 required, 2 editable /
"has default".

- string: `i32 maxlen, u8 flag, varuint len + default bytes` (the length is always
  present, contrary to the spec's "if flag & 4"; verified on ArcGIS-written files)
- objectid: `u8 width (4|8), u8 flag`
- geometry: `u8 0, u8 flag, i16 wktLen (bytes), wkt (UTF-16LE), u8 flags (bit1 Z, bit2 M)`,
  `f64 xorig, yorig, xyscale, [morig, mscale], [zorig, zscale], xytol, [mtol], [ztol]`,
  `f64 xmin ymin xmax ymax, [zmin zmax if layer Z], [mmin mmax if layer M]`,
  `u8 0, u32 ngrid, f64 × ngrid`. Note M before Z in origins, Z before M in extent.
- binary: `u8, u8 flag`; GUID/GlobalID: `u8 38, u8 flag`; XML: `u8 0, u8 flag`
- raster: `u8, u8 flag, u8 len, utf16 name, i16 wktLen, wkt, u8 flags` then (unless
  flags = 0) origins/scales/tolerances as geometry, then `u8 raster_type`
- others: `u8 width, u8 flag, u8 len + default bytes` (length always present)

## Rows
At the offset from `.gdbtablx`: `i32 blob size`, null bitmap `ceil(nNullable/8)` bytes
(bit set = null, one bit per nullable field in field order, LSB first; absent if
none nullable), then values of present fields. OBJECTID is not stored.
String/XML/binary: `varuint len + bytes`. GUID: 16 bytes, printed
`{b3b2b1b0-b5b4-b7b6-b8b9-b10..b15}`. Datetime/DateOnly: f64 days since
1899-12-30. TimeOnly: f64 day fraction. DateTimeWithOffset: f64 days + i16 offset
minutes. Geometry: `varuint len + blob`.

## .gdbtablx
`i32 version, i32 n1024BlocksPresent, i32 nRows (v3) | 0 (v4), i32 offsetSize (4/5/6)`,
then `offsetSize × 1024 × n1024BlocksPresent` offsets (0 = deleted). Trailer (v3) at
the end of offsets: `i32 nBitmapWords, i32 n1024BlocksTotal, i32 nPresent, i32 nUseful`
+ bitmap (bit set = block present). FID = row index + 1; with a bitmap, a row's slot
is `blocksPresentBefore × 1024 + row % 1024`. v4 trailer: `i64 nRows (max OBJECTID), i32 size`, then
`size` bytes of sparse map; a dense v4 file (as ArcGIS writes it, and GeoForge) has `size = 0` and
then `i64 lastBlock` (blocks − 1; −1 for an empty table, which has no blocks). The v4 header word at
offset 4 is 1 when the table has deleted rows, else 0; ArcGIS uses offset size 5 even for small files.

## Geometry blob
`varuint geomType` (low byte = shape type, as in shapefiles; general types 50–54 with
flags 0x80000000 Z, 0x40000000 M, 0x20000000 curves). Coordinates are scaled
integers: `x = v / xyscale + xorigin`.
- Point: `varuint x, y, [z], [m]`, each `(v - 1) / scale + origin`; 0 = empty / NaN.
- Multipoint: `varuint npts`, bbox (4 varuint, max relative to min), then varint
  deltas for x/y (running across all points), then Z deltas, then M deltas.
- Polyline/Polygon: `varuint npts, nparts, [ncurves if curves]`, bbox, `nparts-1`
  part point counts, XY deltas (not reset per part), Z deltas, M deltas (a single
  `0x42` byte instead means all M are NaN; also in multipoints), then curve segments.
  ArcGIS writes the marker even for a one-vertex shape, where it is ambiguous with
  a one-byte M delta: GDAL (3.x) reads it as M = origin − 2 × resolution
  (−100000.0002 by default), ArcGIS as NaN.
- MultiPatch (54): `varuint npts, extended-buffer size, nparts`, bbox, part counts,
  `nparts` part types (low 4 bits), XY deltas, Z deltas, [M].
- Curve segment: `varuint startVertex, varuint type` (1 arc, 4 bezier, 5 elliptic),
  then raw little-endian: arc `f64 x, f64 y, i32 bits` (center, or interior point if
  bits & 0x80); bezier `4 × f64`; elliptic `5 × f64, i32 bits`. (Verified on data; the
  start/type are *not* int32 as in the extended shape buffer.)
- Polygon rings: clockwise = outer, counter-clockwise = inner, holes follow their shell.

## Indexes (.atx attribute, .spx spatial)
Verified against brute-force scans of every index in the GDAL test data.
- `.gdbindexes`: `u32 n`, then per index `u32 len, utf16 name, i16, i32, i16, i32,
  u32 len, utf16 expression, i16`. Attribute index file: `aXXXXXXXX.<name>.atx`.
- Trailer (last 22 bytes v1 / 30 bytes v2): `u8 keySize, u8 kind, i32 1, u32 depth,
  u32|i64 count, i32|i64 0, i32 version`. Pages: 4096 (v1) / 65536 (v2) bytes,
  numbered from 1. **Page 1 is the root.** Version 2 belongs to 64-bit OBJECTID tables (their `.spx`
  and `.atx` alike): `next` and ids are 8 bytes, leaf header `next, u32 n, 8 bytes 0` (20), keys at
  `20 + 4094 × 8` for 8-byte keys (verified on ArcGIS-written files).
- Internal page: `next, u32 n`, then `n + 1` child page ids, values at
  `valuesOffset` = max key under each of the first `n` children.
  Leaf page: `next, u32 n, unknown`, OBJECTIDs, values at `valuesOffset`.
  `valuesOffset = header + maxPerPage × oidSize`, `maxPerPage = (pageSize - header) /
  (oidSize + keySize)`; header 12/20 (leaf), 8/12 (internal).
- Keys: integers/floats native; dates/DateOnly f64 days (legacy dates keep their
  sub-second part even though ArcGIS shows whole seconds); TimeOnly f64 day
  fraction; DateTimeWithOffset f64 days in **UTC**; strings UTF-16 padded with
  spaces to the key size (max 80 chars); GUID/GlobalID **ASCII** `{...}` (38 bytes).
- Spatial key: `grid << 62 | (floor(x / gridSize) + 2^29) << 31 | (floor(y / gridSize) + 2^29)`,
  sorted as signed i64; a feature has one entry per cell its envelope covers, per
  level it is indexed at. A grid size of 0 means the index is unusable (all keys 0).

## Writing attribute indexes (verified byte for byte against ArcGIS Pro 3.7)
- Entry in `.gdbindexes` after `FDO_OBJECTID` (and `FDO_SHAPE`): name, `i16 0, i32 2, i16 0,
  i32 1`, expression = the field name. A UNIQUE index writes the same bytes as a plain one.
- Keys (one per non-null value; nulls are not indexed; empty text is): Short/Long/BigInteger as
  native little-endian integers (key size 2/4/8, trailer kind byte `0x40`); Float/Double and all
  date types (days as f64; DateTimeOffset in UTC) as native floats (4/8, kind `0x60`); text as
  UTF-16LE padded with spaces to `2 × min(length, 80)` bytes, compared by UTF-16 code units, case
  sensitive (kind `0x20`); GUID/GlobalID as the 38 ASCII characters (kind `0`). Ties by OBJECTID.
- Trailer: `u8 keySize, u8 kind, i32 1, u32 depth, u32 count, i32 0, i32 1`.
- Pages: leaves hold `(4096 - 12) / (4 + keySize)` entries, filled in order. An internal page
  holds `per_leaf` keys and `per_leaf + 1` child ids (keys at the same offset as in leaves),
  key `j` = last key under child `j`. **Consecutive internal pages of a level share a boundary
  child** (the last child of one is the first of the next), each page's key in its parent is its
  last stored key, and pages of one level are chained through `next`.
- Page numbers: root = 1, then all leaves in order, then each internal level bottom-up.
  Checked with 300 000 rows (three levels) on Long, Double and Text keys.

## Editing existing geodatabases (observed through ArcGIS; work in progress)
Adding a table touches `a1` (row), `a4` (item row with its definition XML), `a6` (relationship
row), the `.atx` indexes of those three, a new `a4.freelist`, and the `timestamps` file (a small
counter table). Row updates append the new row and put the old space on the table's `.freelist`
(created on first use); deletes zero the `.gdbtablx` entry. Adding fields or indexes changes the
table's own `.gdbtable` / `.gdbindexes`; the catalog XML does not list them. See the experiments
in the session notes.

- `Shape_Length` / `Shape_Area`: ordinary Double fields after the geometry, named after it, nullable in
  the field section (ArcGIS reports them required). ArcGIS keeps them equal to the planar length (perimeter)
  and area of the stored geometry (true arcs for curves; checked to 1e-7 on ArcGIS buffers). GeoForge recomputes
  them when an edit of an existing table inserts a row or changes its geometry, and keeps them otherwise.
- Delete Rows keeps the OBJECTID sequence (the `.gdbtablx` row count stays); Truncate Table restarts it at 1.

## Editing in GeoForge (crash safety)
Every edit is a transaction with a rollback journal in `<gdb>/.gf-journal/` (`src/writer/txn.rs`): before a
file changes for the first time, the manifest records (and syncs) how to undo it: `moved <name> <backup>`
(the original was moved into the journal), `created <name>`, or `grown <name> <length> <backup>` (rows are
appended to a `.gdbtable` in place: its length and its header + field section are kept). Commit syncs the files
and deletes the manifest; a manifest left behind is rolled back, newest entry first, when GeoForge next opens the
geodatabase. The manifest names the owning process, so a journal of a running process is left alone.

## Writing (what GeoForge produces)
Mirrors GDAL's OpenFileGDB writer, which ArcGIS opens: system tables a1–a7 with the
fixed rows in `src/writer/system_rows.rs`, `gdb` = `05 00 00 00 DE AD BE EF`,
`timestamps` = 400 × `FF`, field section at offset 40, layer flags `0x100 | 0x200 |
geomType | M<<30 | Z<<31`, dense `.gdbtablx` v3, `.gdbindexes` with FDO_OBJECTID
(and FDO_SHAPE + single-level `.spx`). XY resolution: 1e-9 for geographic, 1e-4 with
origin -2147483647 otherwise (ArcGIS defaults); Z/M origin -100000, scale 10000. Systems ArcGIS knows get
its own XY domain from `src/xy_domains.tsv.gz` (below).

## Schema items (observed through ArcGIS Pro 3.7; written and edited by `src/writer`)
Everything below lives in `GDB_Items` rows (`Definition` XML), `GDB_ItemRelationships` and, for classes with a
table, `GDB_SystemCatalog`. Item type GUIDs by prefix: feature class `70737809`, table `CD06BC3B`, feature
dataset `74737149`, relationship class `B606A7E1`, coded/range domain `8C368B12` / `C29DA988`; relationship
types: dataset in folder `DC78F1AB`, dataset in feature dataset `A1633A59`, domain in dataset `17E08ADB`,
datasets related through `725BADAB`.
- **Feature dataset**: an item with path `\ds`, `DEFeatureDataset` XML carrying the coordinate system and XY domain,
  a root-folder link; its classes have path `\ds\name`, link only to the dataset (`A1633A59`), and take the dataset's
  coordinate system and domain. Creating the dataset registers its system in `GDB_SpatialRefs`.
- **Subtypes** are definition XML only: `SubtypeFieldName`, `DefaultSubtypeCode`, `Subtypes` with per-subtype
  `SubtypeFieldInfo` (domain, default). The subtype field's `DefaultValueNumeric` follows the default code (XML only;
  the table file's default changes with `AssignDefaultToField`). `GPFieldInfoExs` lists, in table order, only fields
  with something to say; an entry that loses its domain and has nothing else is dropped, one that loses its default stays.
  A new subtype starts with an entry for every field that has a default. Removing a subtype's domain removes its whole
  entry. A domain used by a table (field or subtype) is linked to it (`17E08ADB`).
- **Relationship classes**: an item (`DatasetSubtype1` 1/2/3 = one to one/many/many to many, `DatasetSubtype2` 1 when it has
  a table) linked to the root folder and, through `725BADAB`, to origin and destination. One with a table (attributed or many to
  many) also lists that table in the catalog: fields `<origin fk>, <destination fk>, RID` (RID the object ID, last), the
  two keys indexed `GDB_<DSID>_<field>`, and `DestinationClassKeys` in the definition. Deleting a class deletes the
  relationship classes that use it (and their tables).
- **GDAL reads a relationship class only if the definition's first node is its root element**: an `<?xml ... ?>`
  declaration at the start hides it (GDAL then logs "Cannot find root 'Relationship' node").
- **Delete/Rename** change catalog name (and leave a gap in its OBJECTIDs), item `Name`/`PhysicalName` (upper case)/`Path`,
  and the names inside definitions (`CatalogPath`, `Name`, an empty table `AliasName` becomes the new name, class names in
  relationship classes, `DomainName`); a dataset's children move with it; documentation and item info are untouched.
- **XY domains** (`xy_domains.tsv.gz`, from `scripts/make-xy-domains.py`): a domain's extent is always 2^53 / scale, so the
  origin, resolution and tolerance per coordinate system are all there is; the origin comes from ArcGIS's projection
  engine (Mercator reaches about ±89°, sinusoidal 89.84°) and cannot be derived by a formula.
