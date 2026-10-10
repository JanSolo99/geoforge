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
