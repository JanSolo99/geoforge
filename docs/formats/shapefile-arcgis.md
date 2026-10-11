# Shapefile behavior of ArcGIS Pro 3.7

Observed through the ArcGIS bridge (`tools/arcgis-bridge`); GeoForge matches it.

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
  (r = 10 m). GeoForge spaces partial arcs evenly with the circle rule; shapes agree
  within 0.2 m.
- FLAT line ends are the ROUND buffer minus the end half-discs, even where other
  parts of the line's buffer reach into them.

## Null shapes
ArcGIS writes a feature without a shape (Copy Features, Repair Geometry's KEEP_NULL)
not as a type-0 null record but as an empty record of the layer's type: for a
polyline or polygon, the type, a NaN bounding box and zero parts and points (44
bytes). arcpy reads both as `None`. GeoForge reads such a record as an empty
geometry and still writes nulls as type 0.
