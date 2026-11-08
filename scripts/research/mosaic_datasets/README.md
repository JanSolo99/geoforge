# Mosaic datasets in a file geodatabase — why the tools that read them are deferred

Probed 2026-11-08 with ArcGIS Pro 3.7 on `testdata/geoproc/md_items.gdb` (built with
`CreateMosaicDataset` EPSG:32633 + `AddRastersToMosaicDataset` of `dem.tif`, `dem_nodata.tif`,
`classes_u8.tif`, no overviews; item names are the file stems `dem`, `dem_nodata`, `classes_u8`).

## Blocker: the item table cannot be read

A mosaic dataset `md` is the tables `AMD_md_CAT` (items and footprints), `AMD_md_BND` (boundary),
`AMD_md_CSL` (cell size levels), `AMD_md_SLN` (seamlines, absent until built), `AMD_md_ART`,
`AMD_md_DEF`, `AMD_md_LOG`. `AMD_md_CAT` has a `Raster` column of raster type 2 (inline) and a
`Uri` blob. `unigis-filegdb` fails on its rows (`count 56 exceeds remaining data`) and GDAL's
OpenFileGDB reads them misaligned too (Raster shown as the item name, `MinPS` garbage). Row 1 of
the table (`a0000000b.gdbtable` offset 600): null bitmap `00`, then a 57-byte run of geometry-like
varints whose first byte is `0x20`, then `0a "classes_u8"` (the Name), then doubles 300, 30, 30 and the
Category int: the geometry length prefix and where the inline Raster value sits are not understood
(the geometry blob is not the 32 bytes its prefix says; MinPS = 100 is not where a double is
expected). The footprint geometry and every column need this decoded first.

## Export Mosaic Dataset Geometry (behaviour established, nothing implemented)

- `geometry_type` FOOTPRINT (default) = `AMD_md_CAT` rows without `Raster` and `Uri`; BOUNDARY =
  `_BND`; LEVEL = `_CSL`; SEAMLINE = `_SLN` (ERROR 000210 when there are no seamlines). COVERAGE and
  RESOLUTION are not members (ERROR 000800: FOOTPRINT | BOUNDARY | SEAMLINE | LEVEL).
- Output fields: footprints Name, MinPS, MaxPS, LowPS, HighPS, Category, Tag, GroupName,
  ProductName, CenterX, CenterY, ZOrder, TypeID, ItemTS, UriHash (+ OBJECTID/Shape/Shape_Length/
  Shape_Area); boundary only the shape fields; level InstanceID, LevelID, CellSize, MinPS, MaxPS,
  Category. OBJECTIDs are kept.
- `where_clause`: it must select at least one item of the footprint table (else, or for a field the
  footprint table lacks such as `LevelID`, `ERROR 000210: Cannot create output`), and is then
  evaluated on the exported table: `OBJECTID = 3` with LEVEL or `OBJECTID = 2` with BOUNDARY gives
  an empty output (`WARNING 000117`), `MinPS = 0` with LEVEL one row.
- A non-mosaic input: `ERROR 000732: Mosaic Dataset: Dataset … does not exist or is not supported`.
- Resuming: implement a `mosaic_part` host hook opening `AMD_<name>_<part>`, with `OBJECTID` usable
  in `WhereClause` (it is not a field there), after the reader decodes `AMD_md_CAT`.

The other mosaic tools of the queue (Export Mosaic Dataset Items / Paths, Import Mosaic Dataset
Geometry, Export Frame And Camera Parameters, Remove Rasters From Mosaic Dataset, ...) read or edit
the same catalog.
