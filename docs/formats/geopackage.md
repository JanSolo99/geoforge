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
