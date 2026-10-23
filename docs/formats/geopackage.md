# GeoPackage as ArcGIS Pro 3.7 reads it

Observed black-box with `testdata/gpkg/sample.gpkg` (written by GDAL,
`scripts/make-gpkg-data.py`); reference in `testdata/gpkg/sample.arcgis.json`.

## Reading

- Tables are listed as `main.<table>` (`sample.gpkg/main.points`); only tables in
  `gpkg_contents` with `data_type` `features` or `attributes`.
- The INTEGER PRIMARY KEY column (GDAL: `fid`) is the ObjectID, 64-bit (length 8).
- Column types:

  | Declared type          | ArcGIS field            |
  |------------------------|-------------------------|
  | BOOLEAN, TINYINT, SMALLINT | Short               |
  | MEDIUMINT              | Long                    |
  | INTEGER / INT          | Big Integer             |
  | FLOAT                  | Float                   |
  | REAL, DOUBLE           | Double                  |
  | TEXT(n)                | Text, length n          |
  | TEXT                   | Text, length 2147483647 |
  | DATE                   | Date Only               |
  | DATETIME               | Date (precision 1)      |
  | BLOB                   | Blob                    |

- Shapes: lines read as polylines and polygons as polygons whether single or
  multi; outer rings come out clockwise (Esri), a reversed ring keeps its start
  vertex. Empty points stay empty points; other empty shapes read as null.
- Geographic coordinates are snapped to the spatial reference's resolution on
  read (15.5 reads as 15.500000000000057).

## Writing to shapefiles

- Date Only fields fail the tool with ERROR 000308 (from any source, also file
  geodatabases); Big Integer too; Blob is dropped with WARNING 000592.
- Empty points are written as −DBL_MAX coordinates (also from shapefiles and
  geodatabases), which ArcGIS reads back as empty and GDAL as a real point.
- The 64-bit ObjectID makes some tools refuse a shapefile output:

  | Tool                                               | Result        |
  |----------------------------------------------------|---------------|
  | Copy Features, Select, Feature Class To Feature Class | runs       |
  | Clip                                               | runs for points; ERROR 010672 for lines and polygons |
  | Project                                            | runs, adds a `fid_` Long field |
  | Buffer, Pairwise Buffer, Dissolve, Intersect, Erase, Feature To Point, Multipart To Singlepart | ERROR 010672 |
  | Export Features                                    | ERROR 002809 (OBJECTID) |
  | Spatial Join                                       | ERROR 002809 (TARGET_FID) |

## Writing (Copy Features into a GeoPackage made by Create SQLite Database)

- New file: GeoPackage 1.4 (`application_id` GPKG, `user_version` 10400), page size 4096,
  with all gpkg_* tables (tile and metadata ones too), the tile-matrix triggers,
  `sqlite_sequence`, and spatial references -1, 0 and 4326 (`crates/unigis-gpkg/src/base.rs`).
- Feature table: `CREATE TABLE [t] ([OBJECTID] INTEGER primary key autoincrement not null,
  [Shape] MULTIPOLYGON, ...)` with a type check per column:

  | ArcGIS field | Column                      |
  |--------------|-----------------------------|
  | Short        | SMALLINT, range checked     |
  | Long         | MEDIUMINT, range checked    |
  | Big Integer  | INTEGER                     |
  | Float        | FLOAT, range checked        |
  | Double       | DOUBLE                      |
  | Text(n)      | TEXT(n), length checked     |
  | Date         | DATETIME, text `YYYY-MM-DDTHH:MM:SS.0Z` rounded to whole seconds |
  | Date Only    | DATE, text `YYYY-MM-DD`     |
  | GUID         | TEXT(38) collate nocase     |
  | Blob         | BLOB                        |
  | Time Only, Timestamp Offset | ERROR 000308 |

- OBJECTIDs are renumbered from 1; `sqlite_sequence` holds the count.
- Geometry blobs: version 0, little-endian, no envelope, srs_id; ISO WKB with Z/M codes
  (1000/2000/3000); polygons always MULTIPOLYGON with OGC ring order (outer counterclockwise,
  start vertex kept), lines MULTILINESTRING; missing M is NaN. Coordinates are snapped to
  the coordinate system's XY domain (UTM: origin −5120900/−9998100, resolution 0.0001).
- gpkg_contents: identifier = table name, description NULL, extent NULL, last_change in
  milliseconds UTC. gpkg_spatial_ref_sys: srs_id = EPSG code, Esri WKT (degree to 15
  digits), a description from ArcGIS's projection database, Esri-flavoured WKT2.
- Spatial index: `rtree_<t>_Shape` R*Tree with seven triggers using ST_* functions and a
  `gpkg_rtree_index` extension row (write-only). Node blobs are 1228 bytes (51 entries).
