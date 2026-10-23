# Shapefile behavior of ArcGIS Pro 3.7

Observed through the ArcGIS bridge (`tools/arcgis-bridge`); UniGIS matches it.

## Reading `.dbf` fields
| dBASE | ArcGIS type | precision / scale reported |
|---|---|---|
| `N` width 1–4, 0 decimals | SmallInteger | width / 0 |
| `N` width 5–10, 0 decimals | Integer | width / 0 |
| `N` width ≥ 11, 0 decimals | Double | width / 0 |
| `N` with decimals, width ≤ 8 | Single | width − 1 / decimals |
| `N` with decimals, width ≥ 9 | Double | width − 1 / decimals |
| `F` width ≤ 13 | Single | 0 / 0 |
| `F` width ≥ 14 | Double | 0 / 0 |
| `L` | SmallInteger (T/Y → 1, F/N/? → 0) | 0 / 0 |
| `D` | Date | |
| `C` | String | |

Values: blank or unparseable numbers read as **0**; all-blank text reads as **" "**;
blank or zero dates read as null.

## Writing (Copy Features to shapefile)
| ArcGIS type | dBASE | value format |
|---|---|---|
| Short | `N(5,0)` (reads back as Long) | right-aligned integer |
| Long | `N(10,0)` | right-aligned integer |
| Float | `F(13,11)` | C scientific, 5 mantissa decimals: `1.23457e+04` |
| Double | `F(19,11)` | C scientific, 11 mantissa decimals: `1.79769313486e+308` |
| Text | `C(length)` | left-aligned; null/empty → blanks |
| GUID | `C(38)` | |
| Date | `D(8)` | `YYYYMMDD`; null → 8 blanks |
| Fields read from a shapefile | keep their `N(width, decimals)` | |
| Blob, Raster | dropped — WARNING 000592 | |
| BigInteger, XML, TimeOnly, TimestampOffset | tool fails — ERROR 000308 | |

Null numbers are written as 0. A row with a value that does not fit its column is
skipped: WARNING 160277 "The row contains a bad value", WARNING 000595 lists them.
`.dbf` language driver ID 0; `.cpg` contains `UTF-8`.

## Arc densification in shapefile output (Buffer)
Inside geodatabases ArcGIS keeps true circular arcs (a buffered point is 2 vertices +
one full-circle arc). Writing a shapefile densifies them:
- full circles: `max(36, ceil(π / acos(1 − 0.1/r)))` segments, starting due north,
  clockwise (exact match: 36 up to r = 10 m, 39 at 30 m, 71, 223, 703, 2222 at 100 m–100 km);
- partial arcs (bends, corners, caps) follow a rule we have not pinned down: a 90°
  polygon corner gets 10 segments, a 90° line bend 9, a 180° cap 18, a 170° bend 6
  (r = 10 m). UniGIS spaces partial arcs evenly with the circle rule; shapes agree
  within 0.2 m.
- FLAT line ends are the ROUND buffer minus the end half-discs, even where other
  parts of the line's buffer reach into them.

## Null shapes
ArcGIS writes a feature without a shape (Copy Features, Repair Geometry's KEEP_NULL)
not as a type-0 null record but as an empty record of the layer's type: for a
polyline or polygon, the type, a NaN bounding box and zero parts and points (44
bytes). arcpy reads both as `None`. UniGIS reads such a record as an empty
geometry and still writes nulls as type 0.

## Code pages and memo fields
Without a `.cpg`, ArcGIS decodes `.dbf` text by the language driver ID (byte 29),
including DOS code pages (437, 850, 852, 865, 866 checked); UniGIS uses GDAL's
LDID table (`ENCODING_FROM_LDID`), which agrees. A `.cpg` takes precedence.
ArcGIS cannot open a shapefile whose `.dbf` has a memo (`M`) field, whatever the
version byte (0x03 or 0x83): "cannot open". GDAL and UniGIS read the field as
text holding the `.dbt` block number; the memo text itself is not read.

## Spatial index (`.sbn` / `.sbx`)
Worked out from the files ArcGIS writes (its tool outputs and `AddSpatialIndex` on
point sets of 1–5000 features) with shapelib's `sbnsearch.c` as the outline.
UniGIS's writer reproduces ArcGIS's files byte for byte on every reference output
and on 10k-polygon / 20k-point sets.
- Header (100 bytes): `0x0000270A`, `-400`, 16 zero bytes, file length in 16-bit
  words, record count (null shapes included), the extent as big-endian doubles
  (an empty shapefile's (0, 0, -1, -1) becomes NaN `0x7FF8000000000001`), zeros.
- Record 1, the bin index: big-endian (first bin record number, feature count)
  per tree node, up to the last node holding features; empty nodes are (-1, 0).
- Records 2...: bins of at most 100 features (a node's bins in a row, full ones
  first), 8 bytes per feature: box bytes xmin, ymin, xmax, ymax, then the record
  number (big-endian). Null and empty shapes are left out.
- Box bytes: `floor((v - min) * (255 / (max - min)))` — that exact evaluation
  order; dividing first matters at the maximum — for the minimum, that plus one
  for the maximum, capped at 255 (a point is [v, v + 1]).
- Tree: node `k` has children `2k` (upper half) and `2k + 1` (lower), splitting x at
  odd levels and y at even ones, in byte space [0, 256): a feature goes up if its
  minimum is above the split, down if its maximum is at most the split, else stays.
  Depth: the smallest `L >= 2` with `n <= 8 (2^L - 1)` (thresholds 24, 56, 120,
  248, ...). Below the root, a node whose subtree holds at most 8 features keeps
  all of them (its own first, then its descendants' in node order). ArcGIS still
  keeps about 0.15% of uniformly spread points a level higher than this rule.
  Example (298 points, `f2p_label_polys_inside`): node 2 ends up holding 4 features,
  node 5's own one first, then the three from node 4's subtree, while node 5's deeper 31
  stay in place. Neither "subtree of at most 8" nor merging a node with its children's own
  features (in either child order) reproduces it, and the latter breaks `r2pt_dem_nodata`.
- `.sbx`: the same header (own length), then big-endian (offset, length) in words
  of each `.sbn` record.
