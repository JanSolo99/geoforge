> **Status note.** Generated 2026-10-15 by a review agent from its own knowledge of the
> ArcGIS Pro 3.x tool reference and a read of this repository. Tool names and parameters were
> **not** checked against pro.arcgis.com, and the done/partial/missing marks predate the last
> few commits (Sort, Delete Identical, Add/Delete Field, Calculate Field, Update, Symmetrical
> Difference, field mappings were added after the snapshot). Treat it as a planning map, and
> re-check an item against ArcGIS Pro (the bridge) before building it. The machine-readable
> list is [`gap_list.csv`](gap_list.csv).

# GeoForge gap analysis against ArcGIS Pro 3.x (with an ENVI parity outlook)

Basis: repo state at commit f6a8aff (about 32k lines Rust; 80 `gf` commands; crates core, shapefile, filegdb, geometry, tools, raster, proj, gpkg, json, csv, cli, testkit). ArcGIS tool lists are from knowledge of the Pro 3.x reference (not re-confirmed online). Rows are line items; one row may group several closely related tools, so the counts below are of items, not of individual tools (ArcGIS Pro has roughly 2,000 geoprocessing tools; GeoForge has about 80).

## (a) Executive summary

- GeoForge today is a deep, ArcGIS-verified slice: vector overlay/proximity/statistics tools on shapefile, new file geodatabase and GeoPackage output, Project, JSON/CSV conversion, and a first Spatial Analyst raster core (map algebra basics, focal/zonal, surface, D8 hydrology, rasterization), all compared against Pro 3.7 under Wine. Quality per tool is high (byte-identical dbf/sbn/JSON/CSV in places); breadth is under 5 percent of the tool surface.
- Catalogued items: 469 (58 done, 27 partial, 384 missing). By tool count the missing mass is dominated by: Spatial Analyst (about 250 tools, about 25 done), Image Analyst (about 170 raster functions plus ~60 tools), Spatial Statistics (~60), Network Analyst, 3D Analyst, Geostatistical Analyst, Conversion, Data Management admin (~hundreds), Cartography, and application-level features (map/layout/ModelBuilder/Portal).
- The three structural blockers, in order, are: (1) editing an EXISTING file geodatabase (insert/update/delete rows, add fields, indexes, subtypes, relationship classes, feature datasets) and attribute/schema writes for non-shapefile outputs; (2) the Tool framework (typed params, environments, layers/selections, messages, history) and Python bindings, without which each tool is a bespoke CLI command; (3) a raster stack beyond GeoTIFF (environments cell size/extent/mask/snap, Project Raster, Mosaic, Clip, multi-band, pyramids, Esri Grid/IMG/NetCDF).
- Highest verifiability ratio: vector and table tools, Spatial Analyst, Spatial Statistics, Linear Referencing, 3D Analyst geometry, format round trips. Low or none: deep learning, labeling, rendering pixels, geocoding/online services, Portal, enterprise geodatabase/versioning, random-based tools (EBK, random forest, K-means init).
- ENVI parity is mostly additive on top of Image Analyst (spectral libraries, FLAASH, hyperspectral processing, SARscape, ENVI task API), and ENVI's file format (.hdr/.dat) is trivial to add. There is no ENVI under Wine here, so ENVI-only features need other oracles (published papers, GDAL/open-source implementations as test-only reference, the existing ArcGIS Image Analyst where it overlaps).
- Recommended strategy: finish the foundation (P0), then widen along the most-used ArcGIS workflows (join/field/table/GDB admin, raster management, density/distance/interpolation, spatial statistics), defer GUI/rendering/network/ML.

### Counts per area (items)

| Area | done | partial | missing |
|---|---|---|---|
| Analysis > Extract | 2 | 2 | 3 |
| Analysis > Overlay | 6 | 2 | 10 |
| Analysis > Proximity | 2 | 1 | 9 |
| Analysis > Statistics | 2 | 0 | 2 |
| Data Management > Features | 13 | 3 | 26 |
| Data Management > Fields & Tables | 5 | 2 | 26 |
| Data Management > Workspace / Geodatabase Administration | 0 | 1 | 13 |
| Data Management > Projections and Transformations | 1 | 0 | 10 |
| Data Management > Raster | 1 | 2 | 19 |
| Data Management > Layers, Views, Selections | 0 | 1 | 5 |
| Data Management > Generalization & Simplification | 0 | 0 | 8 |
| Data Management > LAS Dataset / 3D / Point cloud | 0 | 0 | 10 |
| Conversion | 5 | 4 | 17 |
| Cartography | 0 | 0 | 10 |
| Spatial Analyst > Map Algebra / Local / Math | 5 | 0 | 18 |
| Spatial Analyst > Neighborhood / Zonal / Extraction | 3 | 0 | 9 |
| Spatial Analyst > Surface / Hydrology / Distance / Density | 5 | 4 | 27 |
| 3D Analyst | 1 | 0 | 10 |
| Geostatistical Analyst | 0 | 0 | 6 |
| Network Analyst | 0 | 0 | 11 |
| Spatial Statistics | 0 | 0 | 25 |
| Space Time Pattern Mining | 0 | 0 | 4 |
| Image Analyst (and raster functions) | 0 | 1 | 21 |
| Geocoding | 0 | 0 | 3 |
| Linear Referencing | 1 | 0 | 7 |
| Editing tools (geoprocessing + editor) | 0 | 0 | 7 |
| File formats (vector) | 5 | 1 | 14 |
| File formats (raster) | 1 | 1 | 16 |
| arcpy API (Python bindings) | 0 | 3 | 16 |
| Application-level | 0 | 0 | 20 |
| Utility Network (list only) | 0 | 0 | 1 |
| Location Referencing / Business Analyst / Defense (skipped) | 0 | 0 | 1 |

## (b) Per-area gap tables

## Detailed tables

Status: done / partial / missing. Difficulty S (days) / M (1-2 weeks) / L (weeks) / XL (months). Verifiable: Y = ArcGIS Pro under Wine/MCP bridge can produce reference output; P = partly (random, online, GUI output or only approximate comparison); N = not realistically. Priority P0 (foundation / next) to P4 (long term / policy decision).


### Analysis > Extract  (done 2, partial 2, missing 3)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Clip | done | S | Y | P0 | shp/gdb/gpkg; Z/M; matches 0.001 m |
| Select (SQL) | done | S | Y | P0 | SQL subset; BETWEEN rejected as ArcGIS does for shp; gdb/gpkg dialects differ (needs per-workspace SQL) |
| Split | partial | S | Y | P1 | text split field only; gdb output missing |
| Split By Attributes | missing | S | Y | P2 |  |
| Table Select | missing | S | Y | P1 | same engine as Select |
| Spatial Select (Select Layer By Location + Copy) | partial | M | Y | P1 | only relationships via select-by-location; selection types/search distance variants partial |
| Select Layer By Attribute (selection model) | missing | M | Y | P1 | needs layers/selection sets (Phase 6 framework) |

### Analysis > Overlay  (done 6, partial 1, missing 10)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Intersect | done | M | Y | P0 | poly/line/point; line-on-line overlap case open |
| Union | done | M | Y | P0 | cluster_tolerance missing |
| Erase | done | S | Y | P0 |  |
| Identity | done | M | Y | P0 |  |
| Symmetrical Difference | done | M | Y | P0 |  |
| Update | partial | S | Y | P2 | NO_BORDERS missing |
| Spatial Join | done | M | Y | P0 | match options subset? merge rules over several rows partial |
| Feature To Polygon | missing | M | Y | P1 | planar graph build; needed for topology and Polygon from lines |
| Feature To Line | missing | M | Y | P1 | row order banded, unknown |
| Polygon To Line | missing | M | Y | P1 | row order banded, unknown |
| Split Line At Point / Split Line At Vertices | missing | M | Y | P2 |  |
| Count Overlapping Features | missing | M | Y | P2 |  |
| Tabulate Intersection | missing | M | Y | P2 |  |
| Summarize Within | missing | M | Y | P2 |  |
| Summarize Nearby | missing | M | Y | P2 |  |
| Apportion Polygon | missing | M | Y | P3 |  |
| Interpolate Points / Join Features / Aggregate Points | missing | M | P | P3 | some are Pro-only newer tools; check availability |

### Analysis > Proximity  (done 2, partial 1, missing 9)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Buffer | partial | M | Y | P0 | planar done incl. sides/arcs; geodesic missing; multiple ring/line end types partly |
| Multiple Ring Buffer | missing | S | Y | P1 | built on Buffer |
| Near | done | S | Y | P0 | planar; geodesic/location/angle partial |
| Generate Near Table | done | M | Y | P0 |  |
| Point Distance | missing | S | Y | P2 |  |
| Create Thiessen Polygons | missing | M | Y | P1 | Voronoi (needs delaunay) |
| Polygon Neighbors | missing | M | Y | P2 |  |
| Pairwise Buffer | missing | S | Y | P1 | dissolve variants; easy given Buffer |
| Pairwise Clip / Pairwise Intersect / Pairwise Erase / Pairwise Dissolve | missing | S | Y | P2 | alias engines of existing tools but output order/fields differ |
| Graphic Buffer | missing | M | Y | P3 |  |
| Create Buffers (pro-only) | missing | S | Y | P3 |  |
| Near 3D / Generate Near Table 3D | missing | M | Y | P3 |  |

### Analysis > Statistics  (done 2, partial 0, missing 2)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Summary Statistics | done | S | Y | P0 |  |
| Frequency | done | S | Y | P0 |  |
| Statistics (Table Statistics) | missing | S | Y | P2 |  |
| Tabulate Area (Spatial Analyst) | missing | M | Y | P2 |  |

### Data Management > Features  (done 13, partial 3, missing 26)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Copy Features | done | S | Y | P0 |  |
| Feature Class To Feature Class | done | S | Y | P0 |  |
| Feature Class To Geodatabase | missing | S | Y | P1 | batch copy |
| Multipart To Singlepart | done | S | Y | P0 |  |
| Singlepart To Multipart | missing | S | Y | P2 |  |
| Feature To Point | done | M | Y | P0 | incl. INSIDE |
| Feature Vertices To Points | done | S | Y | P0 |  |
| Minimum Bounding Geometry | done | M | Y | P0 |  |
| Points To Line | done | S | Y | P0 |  |
| Add XY Coordinates | done | S | Y | P0 |  |
| Add Geometry Attributes | done | S | Y | P0 |  |
| Dissolve | done | M | Y | P0 | multi-part/single-part; unsplit lines missing |
| Unsplit Line | missing | M | Y | P2 |  |
| Merge | done | S | Y | P0 | field maps partial |
| Merge Branch / Append | partial | S | Y | P1 | Append in place on shp only; schema type TEST/NO_TEST subset |
| Eliminate | missing | M | Y | P2 |  |
| Eliminate Polygon Part | missing | S | Y | P2 |  |
| Integrate | missing | M | Y | P1 | cluster/vertex snapping engine; also underlies topology |
| Snap | missing | M | Y | P2 |  |
| Densify | missing | S | Y | P2 |  |
| Feature Envelope To Polygon | missing | S | Y | P2 |  |
| Feature To 3D By Attribute | missing | S | Y | P3 |  |
| Polygon To Centerline (Cartography) | missing | L | Y | P4 |  |
| Create Features / Create Feature Class / Create Table | missing | S | Y | P0 | needed by scripting and every GDB workflow |
| Create Random Points | missing | S | P | P3 | random: compare statistically/distribution only |
| Create Fishnet | missing | S | Y | P1 |  |
| Generate Points Along Lines | missing | S | Y | P2 |  |
| Generate Tessellation | missing | M | Y | P2 |  |
| Create Spatial Sampling Locations | missing | M | P | P4 | random |
| Check Geometry | done | M | Y | P0 | gdb tables, curves, Z/M problems open |
| Repair Geometry | done | M | Y | P0 |  |
| Delete Features / Delete Rows / Delete | missing | S | Y | P1 |  |
| Truncate Table | missing | S | Y | P2 |  |
| Add Spatial Index / Remove Spatial Index | partial | S | Y | P2 | shp .sbn done; gdb/gpkg index tools missing |
| Calculate Geometry Attributes (legacy) | partial | S | Y | P2 | via Add Geometry Attributes |
| Rotate / Scale / Move / Flip features | missing | S | Y | P3 |  |
| Transform Features (rubbersheet) | missing | L | Y | P4 |  |
| Adjust 3D Z | missing | S | Y | P3 |  |
| Feature Compression, Compress/Uncompress File Geodatabase Data | missing | L | P | P4 | .cdf read/write; proprietary compression format |
| Enable/Disable Editor Tracking | missing | M | Y | P3 | needs existing-GDB edit |
| Enable Attachments / Add Attachments / Generate Attachment Match Table | missing | M | Y | P3 |  |
| Enable Archiving | missing | L | Y | P4 |  |

### Data Management > Fields & Tables  (done 5, partial 2, missing 26)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Add Field | done | S | Y | P0 | shp only; gdb/gpkg writes pending existing-GDB edit support |
| Add Fields (multiple) | missing | S | Y | P1 |  |
| Delete Field | done | S | Y | P0 | shp only |
| Alter Field | missing | S | Y | P1 |  |
| Calculate Field | partial | L | Y | P0 | Python subset; no Arcade, SQL, VB; no comprehension/dict/import; geometry tokens (!SHAPE@!) pending |
| Calculate Fields | missing | S | Y | P1 |  |
| Calculate Field with Arcade | missing | XL | Y | P3 | Arcade interpreter (large, public language spec) |
| Field Mapping / Field Info | partial | M | Y | P0 | fieldmap.rs covers common merge rules |
| Assign Default To Field | missing | S | Y | P3 |  |
| Convert Field | missing | S | Y | P3 |  |
| Join Field | missing | M | Y | P0 | key feature; very common |
| Add Join / Remove Join (layer-based) | missing | M | Y | P1 | needs layer abstraction |
| Add Relate / Create Relationship Class | missing | M | Y | P2 | read of relationship classes done |
| Table To Table | missing | S | Y | P1 |  |
| Table To Geodatabase | missing | S | Y | P1 |  |
| Sort | done | S | Y | P0 |  |
| Delete Identical | done | S | Y | P0 |  |
| Find Identical | done | S | Y | P0 |  |
| Create Table / Make Table View | missing | S | Y | P1 |  |
| Make Query Table | missing | M | Y | P3 |  |
| Add Index / Remove Index | missing | M | Y | P2 | atx index read exists; write missing |
| Calculate Statistics | missing | S | Y | P3 |  |
| Generate Rank / Rank by | missing | S | P | P3 |  |
| Generate Unique ID/GlobalIDs (Add GlobalIDs) | missing | M | Y | P2 | needs GDB edit |
| Create Domain / Add Coded Value To Domain / Table To Domain / Domain To Table | missing | M | Y | P2 | domains read; write needs existing-GDB edit |
| Assign Domain To Field | missing | S | Y | P2 |  |
| Add Subtype / Set Subtype Field / Remove Subtype | missing | M | Y | P2 | XML parse only so far |
| Table Compare / Feature Compare / Raster Compare / File Compare | missing | M | Y | P2 | Data Comparison; useful for GeoForge testing itself |
| Detect Feature Changes | missing | L | Y | P3 |  |
| Create Relationship Class etc. | missing | M | Y | P2 |  |
| Pivot Table / Transpose Fields | missing | M | Y | P2 |  |
| Summarize Attributes / Concatenate / Cross Tabulate | missing | M | Y | P3 |  |
| Text-processing: Parse Path, Create Unique Name, Validate Table Name etc. | missing | S | Y | P2 | arcpy.ValidateTableName etc. |

### Data Management > Workspace / Geodatabase Administration  (done 0, partial 1, missing 13)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create File GDB / Create Folder / Create Feature Dataset | partial | M | Y | P0 | new GDB write only; no dataset write; create tool commands missing |
| Compact / Compress File GDB | missing | L | Y | P2 | needs rewrite of table files |
| Upgrade Dataset / Upgrade Spatial Reference | missing | M | Y | P4 |  |
| Create Enterprise Geodatabase / Mobile GDB | missing | L | P | P3 | mobile GDB = SQLite; enterprise = RDBMS (not Wine-testable) |
| Create Database Connection | missing | M | N | P3 |  |
| Create Mobile Geodatabase | missing | L | Y | P2 | SQLite base reuse (gpkg crate) |
| Rebuild Indexes / Analyze Datasets | missing | S | Y | P4 |  |
| Register/Unregister with Geodatabase | missing | M | P | P4 | enterprise-only |
| Create/Manage Versions, Reconcile, Post | missing | XL | N | P4 | enterprise only; ArcGIS needs SDE server |
| Delete/Rename/Copy/Move (dataset management) | missing | S | Y | P0 | gf rename/copy/delete datasets within GDB |
| Add Rule / Attribute rules (Add Attribute Rule, Evaluate Rules) | missing | XL | Y | P3 | needs Arcade |
| Topology tools (Create Topology, Add Rule To Topology, Validate Topology) | missing | XL | Y | P3 | dirty-area engine, rule set of ~30; verifiable |
| Geometric Network / Create Geometric Network | missing | XL | P | P4 | legacy |
| Parcel Fabric tools | missing | XL | P | P4 | Esri-specific data model |

### Data Management > Projections and Transformations  (done 1, partial 0, missing 10)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Project | done | M | Y | P0 | methods Esri-only missing; vertical datum missing |
| Define Projection | missing | S | Y | P0 | trivial for shp (.prj) , gdb needs table rewrite |
| Batch Project | missing | S | Y | P1 |  |
| Create Custom Geographic Transformation | missing | M | Y | P2 |  |
| Create Spatial Reference | missing | S | Y | P2 |  |
| Project Raster | missing | L | Y | P0 | resampling methods NEAREST/BILINEAR/CUBIC/MAJORITY |
| Warp / Rectify / Georeference | missing | L | Y | P2 |  |
| Convert Coordinate Notation | missing | M | Y | P2 | DD/DDM/DMS/MGRS/USNG/UTM/GARS/GEOREF |
| Raster Define Projection | missing | S | Y | P1 |  |
| Flip/Mirror/Rotate/Shift/Rescale (raster geometry) | missing | S | Y | P2 |  |
| Reproject on-the-fly (map) | missing | M | N | P3 | renderer-level |

### Data Management > Raster  (done 1, partial 2, missing 19)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Resample | done | S | Y | P0 |  |
| Clip (raster) | missing | S | Y | P0 |  |
| Mosaic / Mosaic To New Raster | missing | M | Y | P0 |  |
| Composite Bands | missing | S | Y | P0 |  |
| Extract Bands | missing | S | Y | P0 |  |
| Copy Raster | missing | S | Y | P0 |  |
| Raster To Other Format | missing | S | Y | P1 |  |
| Build Pyramids | missing | M | Y | P1 | overviews |
| Calculate Statistics (raster) | missing | S | Y | P0 | aux.xml histograms partially done |
| Build Raster Attribute Table | missing | S | Y | P1 |  |
| Create Raster Dataset / Create Random Raster | missing | S | Y | P2 |  |
| Split Raster | missing | S | Y | P2 |  |
| Cell Statistics / Band Collection Statistics | missing | M | Y | P1 |  |
| Pansharpen (DM) | missing | M | Y | P2 |  |
| Set Raster Properties / Delete Raster Attribute | missing | S | Y | P2 |  |
| Shaded Relief | missing | M | Y | P2 |  |
| Create Mosaic Dataset / Add Rasters To Mosaic Dataset / Build Seamlines etc. | missing | XL | Y | P3 | Mosaic dataset is an Esri GDB object |
| Create Raster Catalog | missing | M | P | P4 | legacy |
| Compute Pansharpen Weights, Color Balance | missing | L | Y | P3 |  |
| Image Compression: LZ77, JPEG, JPEG2000, LERC, ZSTD | partial | L | Y | P2 | LZW/Deflate/PackBits done |
| Convert Raster Format to CRF | missing | XL | P | P3 | CRF/MRF are open (MRF) or Esri-private (CRF); reverse-engineer |
| Mask / Nodata handling environments | partial | M | Y | P0 | cell size/snap/extent/mask envs open |

### Data Management > Layers, Views, Selections  (done 0, partial 1, missing 5)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Make Feature Layer / Make Table View / Make Raster Layer / Make XY Event Layer | missing | M | Y | P1 | layer = dataset + def query + selection + fields; central abstraction for the Python API |
| Select Layer By Attribute / Select Layer By Location | partial | M | Y | P1 | location only, no selection persistence |
| Save To Layer File / Apply Symbology From Layer / Add Join | missing | L | Y | P2 | .lyrx JSON |
| Get Count / Get Raster Properties / Get Field Value / Get Cell Value | missing | S | Y | P0 | small, used by scripts |
| Copy Rows / Delete Rows | missing | S | Y | P0 |  |
| Calculate Value / Iterate Features (ModelBuilder) | missing | M | P | P3 |  |

### Data Management > Generalization & Simplification  (done 0, partial 0, missing 8)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Simplify Line (Cartography) | missing | M | Y | P2 | Douglas-Peucker/point remove/bend simplify; bend simplify proprietary-ish |
| Simplify Polygon | missing | M | Y | P2 |  |
| Smooth Line / Smooth Polygon (PAEK, BEZIER) | missing | M | Y | P2 | PAEK documented |
| Thin Road Network | missing | L | Y | P4 |  |
| Aggregate Points / Aggregate Polygons | missing | M | Y | P3 |  |
| Dissolve Boundaries / Collapse Dual Lines | missing | L | Y | P4 |  |
| Eliminate / Eliminate Polygon Part | missing | M | Y | P2 |  |
| Delineate Built-Up Areas, Resolve Building Conflicts | missing | XL | P | P4 |  |

### Data Management > LAS Dataset / 3D / Point cloud  (done 0, partial 0, missing 10)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| LAS reader/writer (LAS 1.0-1.4, LAZ, zLAS) | missing | L | Y | P1 | LAZ needs LASzip compatible decoder (open spec); zLAS Esri-specific |
| Create LAS Dataset | missing | M | Y | P2 | .lasd XML |
| LAS Dataset To Raster | missing | M | Y | P1 |  |
| LAS To Multipoint | missing | S | Y | P1 |  |
| Classify LAS Ground / Noise / Building / Overlap / By Height | missing | L | Y | P2 | algorithms partially documented; matching exact classification hard |
| Extract LAS | missing | S | Y | P2 |  |
| Point Cloud To Raster / Multipoint | missing | M | Y | P2 |  |
| Convert LAS / Tile LAS / Thin LAS | missing | M | Y | P2 |  |
| Locate Outliers, Colorize LAS | missing | M | Y | P3 |  |
| Point cloud scene layers (I3S, SLPK) | missing | XL | P | P4 |  |

### Conversion  (done 5, partial 4, missing 17)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Features To JSON / JSON To Features | done | M | Y | P0 | Esri JSON & GeoJSON |
| Export Table (csv) | done | S | Y | P0 |  |
| Export Table to dbf/gdb/gpkg | partial | S | Y | P1 | csv, gpkg, dbf; gdb partially |
| Table To Excel / Excel To Table | missing | M | Y | P1 | xlsx read+write (no external dep) ; sheet naming and type inference |
| XY Table To Point | done | S | Y | P0 |  |
| Export Features | partial | S | Y | P0 | alias of Copy Features with where_clause |
| Feature Class To Shapefile / Table To dBASE | partial | S | Y | P1 | batch wrappers |
| Raster To Point / Polygon | done | M | Y | P0 | SIMPLIFY missing |
| Raster To Polyline | missing | M | Y | P1 |  |
| Raster To ASCII / ASCII To Raster / Float To Raster / Raster To Float | missing | S | Y | P1 | simple formats |
| Feature To Raster / Polygon To Raster / Polyline To Raster / Point To Raster | done | M | Y | P0 |  |
| Points To Raster (Point to raster stats) | missing | S | Y | P2 |  |
| Raster To Other Format (TIFF/IMG/JPEG/PNG/BMP/GIF) | partial | M | Y | P1 | GeoTIFF only |
| Export Raster Data / Raster To Video | missing | M | P | P4 |  |
| KML To Layer / Layer To KML / Map To KML | missing | M | Y | P2 | KML XML + KMZ zip; symbology to KML |
| Import CAD / CAD To Geodatabase (DWG, DXF, DGN) | missing | XL | Y | P3 | DXF open text; DWG is closed -> reverse-engineer policy |
| GML / WFS To Features / OGC (WMS, WCS) | missing | L | P | P3 | WFS needs network |
| Import XML / Export XML Workspace Document | missing | M | Y | P2 |  |
| Metadata tools (Import Metadata, Export Metadata, Upgrade Metadata, FGDC/ISO 19139/19115) | missing | L | Y | P2 | incl. .shp.xml |
| Multidimensional: NetCDF To Raster/Feature/Table, Make NetCDF Layers, HDF, GRIB | missing | L | Y | P2 | netcdf-c or own HDF5 reader; no Rust HDF5 dependency policy needed |
| Feature Class To CAD / Export CAD | missing | L | P | P4 |  |
| Excel / Parquet / Arrow (Pro 3.x Parquet support) | missing | M | P | P3 |  |
| Table To Table to SQLite/Database | missing | M | P | P4 |  |
| Layer To KML | missing | M | P | P3 |  |
| Raster To LAS and LAS To Raster | missing | M | Y | P2 |  |
| OSM / Open Street Map (Load OSM) | missing | M | N | P4 |  |

### Cartography  (done 0, partial 0, missing 10)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Generalization (Aggregate Polygons, Simplify, Smooth, Collapse Road Detail) | missing | L | Y | P3 |  |
| Masking (Feature Outline Masks, Polygon Mask) | missing | M | Y | P3 |  |
| Graphic Conflicts (Resolve Road Conflicts, Detect Graphic Conflict) | missing | XL | P | P4 | solver is proprietary |
| Grids and Graticules (Grid, Graticule, Measured Grid, Reference Grid) | missing | M | Y | P2 | documented geometry |
| Annotation (Convert Labels To Annotation, Feature To Annotation) | missing | XL | Y | P4 | needs label engine |
| Cartographic Partitions, Data Driven Pages (Build Cartographic Partition) | missing | M | Y | P3 |  |
| Symbology tools (Apply Symbology From Layer, Add Colormap, Create Cartographic Partitions) | missing | L | Y | P3 |  |
| Representations / Cartographic Representation tools | missing | XL | P | P4 | rep rules storage |
| Tiling schemes: Generate Map Server Cache Tiling Scheme, Create Map Tile Package | missing | XL | P | P3 |  |
| Create Vector Tile Package / Index | missing | XL | P | P3 | MVT (public spec) + Esri styles |

### Spatial Analyst > Map Algebra / Local / Math  (done 5, partial 0, missing 18)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Plus Minus Times Divide Int Float | done | S | Y | P0 |  |
| Abs Exp Exp2 Exp10 Ln Log2 Log10 Sqrt Square Power Mod | missing | S | Y | P0 |  |
| Sin Cos Tan Asin Acos Atan Atan2 Sinh Cosh Tanh | missing | S | Y | P1 |  |
| And Or Not Xor Bitwise (BitwiseAnd...) | missing | S | Y | P1 |  |
| Equal To, Greater Than, Less Than (+ Frequency) | missing | S | Y | P0 |  |
| Boolean And / Or | missing | S | Y | P1 |  |
| Round Down / Round Up / Negate / Ceil/Floor | missing | S | Y | P1 |  |
| Is Null | done | S | Y | P0 |  |
| Con | done | S | Y | P0 |  |
| Set Null | done | S | Y | P0 |  |
| Pick | missing | S | Y | P1 |  |
| Cell Statistics (local) | missing | M | Y | P0 |  |
| Combine | missing | M | Y | P1 |  |
| Equal To Frequency etc. | missing | S | Y | P2 |  |
| Highest Position / Lowest Position | missing | S | Y | P2 |  |
| Popularity / Rank / Majority / Minority / Variety | missing | S | Y | P2 |  |
| Lookup | missing | S | Y | P1 |  |
| Raster Calculator (expression parser) | missing | M | Y | P0 | syntax in map algebra; leverage pycalc |
| Reclassify (cell value) | done | S | Y | P0 |  |
| Reclass by Table / Slice / Rescale by Function | missing | M | Y | P1 |  |
| Raster Calculator (Python Raster class operator overloading) | missing | M | Y | P1 | Python bindings (Phase 6) |
| Fuzzy Membership / Fuzzy Overlay | missing | M | Y | P2 |  |
| Weighted Overlay / Weighted Sum | missing | S | Y | P1 |  |

### Spatial Analyst > Neighborhood / Zonal / Extraction  (done 3, partial 0, missing 9)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Focal Statistics | done | M | Y | P0 |  |
| Block Statistics | missing | S | Y | P1 |  |
| Filter (LOW/HIGH) | missing | S | Y | P1 |  |
| Line Statistics / Point Statistics | missing | M | Y | P2 |  |
| Focal Flow | missing | S | Y | P3 |  |
| Zonal Statistics + as Table | done | M | Y | P0 |  |
| Zonal Fill / Zonal Geometry (+ as Table) / Zonal Histogram | missing | M | Y | P1 |  |
| Tabulate Area | missing | M | Y | P1 |  |
| Extract by Mask | done | S | Y | P0 |  |
| Extract by Attributes / Circle / Points / Polygon / Rectangle | missing | S | Y | P1 |  |
| Extract Values to Points / Sample | missing | S | Y | P1 |  |
| Extract Multi Values to Points | missing | S | Y | P2 |  |

### Spatial Analyst > Surface / Hydrology / Distance / Density  (done 5, partial 4, missing 27)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Slope / Aspect / Curvature | done | M | Y | P0 | planar; geodesic missing |
| Hillshade | partial | M | Y | P0 | shadows missing |
| Contour | done | M | Y | P0 |  |
| Contour List / Contour With Barriers | missing | M | Y | P2 |  |
| Viewshed (classic) | partial | L | Y | P2 | approximate; OFFSETB etc. |
| Viewshed 2 / Observer Points | missing | L | Y | P3 |  |
| Visibility | missing | L | Y | P3 |  |
| Cut Fill / Surface Volume / Area and Volume Statistics | missing | M | Y | P2 |  |
| Aspect | done | M | Y | P0 |  |
| Surface Parameters | missing | M | Y | P2 |  |
| Observer Points | missing | M | Y | P3 |  |
| Fill / Flow Direction (D8) / Flow Accumulation | done | M | Y | P0 | flat handling approximated |
| Flow Direction (MFD, DINF) | missing | L | Y | P2 |  |
| Basin / Watershed / Stream Link / Stream Order / Flow Length | done | M | Y | P0 |  |
| Stream To Feature / Stream Order (Shreve) | missing | M | Y | P1 |  |
| Snap Pour Point | missing | S | Y | P1 |  |
| Sink / Flow Distance / Watershed vs Basin | missing | M | Y | P2 |  |
| Euclidean Distance | partial | M | Y | P0 | source raster only; features/barriers missing |
| Euclidean Direction / Allocation | missing | M | Y | P1 |  |
| Cost Distance / Cost Allocation / Cost Back Link / Cost Path / Corridor / Path Distance / Cost Connectivity | missing | L | Y | P2 | graph of cells |
| Distance Accumulation / Distance Allocation | missing | L | Y | P3 |  |
| Optimal Path As Line / Region Network | missing | L | Y | P3 |  |
| Kernel Density / Point Density / Line Density | missing | M | Y | P1 |  |
| Kernel Density with barriers | missing | L | Y | P3 |  |
| Interpolation: IDW | missing | M | Y | P1 |  |
| Kriging, Natural Neighbor, Spline, Spline with Barriers, Topo To Raster, Trend | missing | L | Y | P2 | Topo To Raster is ANUDEM (documented but intricate) |
| Solar Radiation (Area, Points, Graphics, Sunmap) | missing | XL | Y | P3 | algorithm documented (Fu & Rich) but exact reproduction hard |
| Groundwater (Darcy Flow, Particle Track) | missing | L | Y | P4 |  |
| Multivariate (Class Probability, Principal Components, ISO Cluster, Maximum Likelihood Classification, Create Signatures, Dendrogram, Band Collection Stats) | missing | L | Y | P2 | iso cluster sensitive to init |
| Segmentation and Classification (Segment Mean Shift, Train Random Trees, Train SVM, Classify Raster) | missing | XL | P | P3 | SVM/RF training not bit-exact; use policy |
| Generalization (Aggregate, Boundary Clean, Expand, Majority Filter, Nibble, Region Group, Shrink, Thin, Thin Raster) | missing | M | Y | P2 |  |
| Overlay: Fuzzy, Weighted Overlay, Weighted Sum, Combine | missing | M | Y | P1 |  |
| Raster Creation: Create Constant Raster, Create Random Raster, Create Normal Raster | missing | S | P | P2 | random not exact |
| Raster Reclass: Reclassify, Rescale, Slice, Lookup | partial | M | Y | P0 | Reclassify only |
| Environment settings (cell size, extent, snap, mask, output CS, scratch workspace, parallel processing, compression, pyramid, tile size) | missing | M | Y | P0 | crosscutting |
| Geodesic Slope/Aspect with geographic data | missing | M | Y | P2 |  |

### 3D Analyst  (done 1, partial 0, missing 10)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| TIN Create / Edit TIN / TIN To Raster / TIN Domain / TIN Node / TIN Edge / TIN Triangle | missing | L | Y | P2 | Delaunay with Esri constrained rules; TIN file format (tin folder) reverse-engineering |
| Raster To TIN (Raster To TIN uses Z tolerance) | missing | L | Y | P3 |  |
| Terrain datasets (Create Terrain, Build Terrain) | missing | XL | Y | P4 |  |
| Add Z Information / Interpolate Shape / Surface Information / Stack Profile | missing | M | Y | P2 |  |
| Interpolate Polygon To Multipatch / Extrude Between / Skyline / Sky Barrier / Skyline Graph | missing | L | Y | P3 |  |
| Line Of Sight / Construct Sight Lines / Intervisibility | missing | L | Y | P3 |  |
| Features To 3D, Layer 3D To Feature Class, Multipatch tools (Intersect 3D Line With Surface, Is Closed 3D, Replace Profile) | missing | M | Y | P3 |  |
| Multipatch geometry read/write | done | M | Y | P0 | shp + gdb read/write |
| Scene layer packages (SLPK, I3S) | missing | XL | P | P4 | I3S is public spec |
| Raster To Multipoint, Voxel tools | missing | L | P | P4 |  |
| Visibility / Viewshed 3D / Shadow Volume | missing | L | Y | P4 |  |

### Geostatistical Analyst  (done 0, partial 0, missing 6)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Exploratory Spatial Data Analysis (histogram, Voronoi map, QQ plot, semivariogram cloud) | missing | L | N | P3 | UI, no tool |
| Create Geostatistical Layer / GA Layer To Grid / Contour / Points | missing | L | Y | P3 |  |
| Kriging (ordinary, simple, universal, indicator, probability, disjunctive, cokriging, empirical Bayesian) | missing | XL | Y | P3 | Empirical Bayesian Kriging heavy, random |
| IDW, Global/Local Polynomial, RBF, Diffusion/Areal interpolation | missing | L | Y | P2 | IDW Pro-GA variant differs from SA IDW |
| Cross Validation / Validation / Subset Features | missing | M | Y | P3 |  |
| Geostatistical simulation (Gaussian Geostatistical Simulations) | missing | XL | N | P4 | random |

### Network Analyst  (done 0, partial 0, missing 11)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create Network Dataset / Build Network | missing | XL | Y | P2 | .nd format is Esri binary in gdb; need own graph + file support |
| Network dataset XML (Create Network Dataset From Template) | missing | M | Y | P3 |  |
| Make Route/Closest Facility/Service Area/OD Cost Matrix/Location-Allocation/VRP layer | missing | XL | Y | P2 | requires own router; Esri uses hierarchies, restrictions, turn tables; locally testable using custom network |
| Solve | missing | XL | Y | P2 |  |
| Add Locations / Add Field To Analysis Layer / Calculate Locations | missing | L | Y | P3 |  |
| Network Analysis arcpy.na module (Route, ServiceArea, ClosestFacility) | missing | L | Y | P3 |  |
| Service Area Polygons generation | missing | L | Y | P3 | trim polygons, detailed vs generalized: proprietary |
| Vehicle Routing Problem solver | missing | XL | Y | P4 | optimization heuristics: results not unique |
| Online network/routing (World service) | missing | XL | N | P4 |  |
| Utility Network (Trace, Validate, Update Subnetwork, Disable ...) | missing | XL | N | P4 | enterprise |
| Network Analyst on public transit (GTFS) | missing | XL | Y | P4 |  |

### Spatial Statistics  (done 0, partial 0, missing 25)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Mean Center / Median Center / Central Feature | missing | S | Y | P1 |  |
| Standard Distance / Directional Distribution (Standard Deviational Ellipse) | missing | S | Y | P1 |  |
| Linear Directional Mean | missing | S | Y | P2 |  |
| Average Nearest Neighbor | missing | S | Y | P1 |  |
| Multi-Distance Spatial Cluster Analysis (Ripley's K) | missing | M | Y | P2 | permutation is random |
| Spatial Autocorrelation (Moran's I) / Incremental Spatial Autocorrelation | missing | M | Y | P1 |  |
| High/Low Clustering (Getis-Ord General G) | missing | M | Y | P2 |  |
| Hot Spot Analysis (Getis-Ord Gi*) | missing | M | Y | P1 | FDR correction |
| Optimized Hot Spot Analysis | missing | L | Y | P2 |  |
| Cluster and Outlier Analysis (Anselin Local Moran's I) | missing | M | Y | P1 | permutations random |
| Optimized Outlier Analysis | missing | L | Y | P2 |  |
| Spatial Weights Matrix (Generate Spatial Weights Matrix, Generate Network SWM) | missing | M | Y | P1 |  |
| Spatially Constrained Multivariate Clustering | missing | L | Y | P3 |  |
| Multivariate Clustering (K-Means, K-Medoids) | missing | M | Y | P2 | random init |
| Density-based Clustering (DBSCAN, HDBSCAN, OPTICS) | missing | M | Y | P2 | DBSCAN deterministic |
| Ordinary Least Squares (OLS) / Exploratory Regression | missing | M | Y | P1 | well-defined numerics |
| Geographically Weighted Regression (GWR) / Multiscale GWR | missing | L | Y | P2 |  |
| Generalized Linear Regression (GLR) / Forest-based Classification and Regression | missing | XL | P | P3 | RF random |
| Spatial Lag/Error, Spatial Regression | missing | M | Y | P3 |  |
| Similarity Search / Grouping Analysis / Build Balanced Zones | missing | L | P | P3 |  |
| Calculate Distance Band / Collect Events / Create Spatial Weights Matrix | missing | S | Y | P2 |  |
| Kernel Interpolation With Barriers / Areal Interpolation | missing | L | Y | P3 |  |
| Subset Features / Calculate Composite Index / Trend Detection | missing | M | Y | P3 |  |
| Geographic Cluster Detection (SaTScan-like) / Colocation Analysis | missing | L | Y | P4 |  |
| Dimension Reduction / Neighborhood Summary Statistics | missing | M | Y | P3 |  |

### Space Time Pattern Mining  (done 0, partial 0, missing 4)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create Space Time Cube (from defined locations, from features) | missing | L | Y | P3 | NetCDF cube; need netcdf writing |
| Emerging Hot Spot Analysis / Local Outlier Analysis / Time Series Clustering | missing | L | Y | P3 |  |
| Visualize Space Time Cube / Curve Fit Forecast / Exponential Smoothing / Forest-Based Forecast | missing | L | P | P4 |  |
| Mann-Kendall trend | missing | S | Y | P3 |  |

### Image Analyst (and raster functions)  (done 0, partial 1, missing 21)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Classification: Maximum Likelihood, ISO Cluster, Random Trees, SVM, K-nearest, Classify Raster, Train ... | missing | L | P | P2 | ML models not bit-exact; use deterministic ones (ML, ISO) first |
| Segment Mean Shift | missing | L | Y | P3 |  |
| Accuracy Assessment / Confusion Matrix / Create Accuracy Assessment Points | missing | M | Y | P2 |  |
| Change detection: Compute Change Raster, Detect Change Using Change Analysis Raster, Change Detection Wizard | missing | M | Y | P3 |  |
| Image Analyst raster functions (~170: NDVI, SAVI, Band Arithmetic, Remap, Clip, Convolution, Pansharpening, Hillshade, Colormap, Stretch, Gamma, Statistics, Local, Focal, Mosaic, Mask, Composite, Extract Band, Spectral Conversion, Orthorectification, Fuzzy, ...) | missing | XL | Y | P2 | framework: raster function chain JSON (.rft.xml), on-the-fly evaluation |
| Spectral indices (Compute Spectral Index e.g. NDVI, EVI, NDWI) | missing | S | Y | P1 | formulas documented |
| Band Arithmetic function | missing | S | Y | P1 |  |
| Deep learning tools (Train Deep Learning Model, Detect Objects Using Deep Learning, Classify Pixels, Export Training Data, Segment Anything) | missing | XL | N | P4 | needs PyTorch/TensorFlow runtime + Esri model definition (.emd/.dlpk); Wine ArcGIS lacks CUDA -> verification unlikely |
| Object classification, Object tracking, Super Resolution | missing | XL | N | P4 |  |
| Feature extraction: Raster To Polygon, Raster Object Detection | partial | M | Y | P2 | vectorization done |
| Orthomapping (Create Ortho Mapping Project, Compute Block Adjustment, Generate DEM/DSM/Orthomosaic from drone imagery) | missing | XL | N | P4 | SfM; ArcGIS proprietary |
| Motion imagery / FMV | missing | XL | N | P4 |  |
| Pansharpening (Gram-Schmidt, Brovey, IHS, Esri) | missing | M | Y | P2 | published math; Esri variant details |
| Radiometric: Compute Surface Reflectance, Apparent Reflectance, Dark Object Subtraction | missing | M | Y | P2 |  |
| Raster Analysis (Raster Analytics: distributed on server) | missing | XL | N | P4 | portal |
| Mosaic dataset functions (Seamlines, Color Balance, Cloud Mask) | missing | XL | Y | P4 |  |
| Principal Components, Band Indexes, Hyperspectral Spectral Angle Mapper | missing | M | Y | P2 |  |
| Geometry / terrain: Compute Camera Model, Build Footprints | missing | L | Y | P4 |  |
| Raster Collections, Time-Series Analysis (Generate Trend Raster, Predict using trend, Analyze Changes using CCDC, LandTrendr) | missing | L | Y | P4 |  |
| Multidimensional raster tools (Make Multidimensional Raster Layer, Aggregate, Subset, Generate Multidimensional Anomaly) | missing | L | Y | P3 |  |
| Interpolation of Voids: Fill Voids, Remove Cloud | missing | M | Y | P3 |  |
| Vegetation index surfaces: Tasseled Cap, Thematic | missing | S | Y | P2 |  |

### Geocoding  (done 0, partial 0, missing 3)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create Locator / Create Composite Address Locator | missing | XL | P | P4 | locator .loc format proprietary, address parsing rules |
| Geocode Addresses / Reverse Geocode / Rebuild Address Locator | missing | XL | P | P4 | results depend on reference data and scoring; online World Geocoding is a service |
| Standardize Addresses / Geocode Table | missing | L | P | P4 |  |

### Linear Referencing  (done 1, partial 0, missing 7)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create Routes | missing | M | Y | P2 |  |
| Calibrate Routes | missing | M | Y | P2 |  |
| Locate Features Along Routes | missing | M | Y | P2 |  |
| Make Route Event Layer / Route Event Properties | missing | M | Y | P2 |  |
| Overlay Route Events / Dissolve Route Events / Transform Route Events / Fabricate Routes | missing | L | Y | P3 |  |
| Create Route Event Tables | missing | M | Y | P3 |  |
| M-aware geometry (read/write) | done | M | Y | P0 |  |
| Location Referencing (LRS) | missing | XL | N | P4 | skip - Roads and Highways |

### Editing tools (geoprocessing + editor)  (done 0, partial 0, missing 7)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Append / Merge / Divide / Cut / Union / Planarize / Reshape / Trim / Extend / Fillet / Generalize | missing | M | Y | P3 | editor sketch ops (Python via arcpy edit not supported); geometry engine ops exist |
| Edit session semantics (arcpy.da.Editor, Start/Stop Editing, undo/redo) | missing | L | Y | P2 |  |
| Feature templates / Construction tools | missing | XL | N | P4 | GUI |
| Snapping environment / Attribute Transfer / Align Features | missing | M | Y | P3 |  |
| Rubbersheet Features / Edgematch Features | missing | L | Y | P3 |  |
| Subtype/domain/attribute-rule validation on edit | missing | L | Y | P3 |  |
| Topology editing | missing | XL | Y | P4 |  |

### File formats (vector)  (done 5, partial 1, missing 14)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Shapefile (.shp/.shx/.dbf/.prj/.cpg/.sbn/.sbx) | done | - | Y | P0 | .shp.xml, .qix not needed |
| File Geodatabase read | done | XL | Y | P0 | compressed .cdf missing |
| File Geodatabase write new | partial | XL | Y | P0 | add to existing, indexes, subtypes, relationship classes, feature datasets, annotation, networks, topology, rasters, mosaics missing |
| File Geodatabase edit in place (update/delete/insert rows, schema changes) | missing | XL | Y | P0 | key blocker for tools running against existing GDBs |
| Mobile Geodatabase (.geodatabase) read/write | missing | L | Y | P2 | SQLite + ST_Geometry; reuse gpkg sqlite |
| GeoPackage read/write | done | L | Y | P0 | WAL, curves open |
| SpatiaLite (.sqlite) | missing | M | Y | P2 | share SQLite code; SpatiaLite blob format public |
| KML/KMZ | missing | M | Y | P2 |  |
| GML / GeoRSS / WFS | missing | L | P | P3 |  |
| DXF / DWG / DGN | missing | XL | Y | P3 | DXF open spec, DWG/DGN closed (OpenDesign alliance specs public for DWG) |
| CSV / TXT tables | done | S | Y | P0 |  |
| Excel xlsx/xls | missing | M | Y | P1 |  |
| Esri JSON / GeoJSON | done | M | Y | P0 |  |
| TopoJSON, GeoJSONL, Parquet, Arrow, FlatGeobuf, MVT | missing | M | P | P3 | ArcGIS Pro 3.x partial support |
| Coverage / E00 / ArcInfo | missing | L | Y | P4 | legacy; E00 text is simple, Coverage binary |
| PostgreSQL / PostGIS / Oracle / SQL Server / SAP HANA enterprise GDB | missing | XL | N | P4 | need servers; use database connections, ST_Geometry |
| Personal geodatabase (.mdb) | missing | L | N | P4 | Access Jet - no Linux; skip |
| Shapefile .shp.xml metadata | missing | M | Y | P2 |  |
| Annotation, Dimension feature classes, Relationship classes (write) | missing | L | Y | P3 |  |
| Layer files (.lyrx, .lyr) / Project (.aprx) / Map packages (.mpkx) / Layer packages (.lpkx) / Project packages (.ppkx) / Tile packages (.tpkx) / Vector tile (.vtpk) / Scene (.slpk) / Geoprocessing package (.gpkx) | missing | L | Y | P2 | aprx/lyrx JSON/zip formats; many are zips of gdb + json |

### File formats (raster)  (done 1, partial 1, missing 16)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| GeoTIFF read/write (strips/tiles, LZW/Deflate/PackBits, BigTIFF) | done | L | Y | P0 |  |
| GeoTIFF compression JPEG, LERC, ZSTD, WebP | missing | M | Y | P2 |  |
| COG layout & overviews | missing | M | Y | P1 |  |
| .aux.xml statistics/histograms | partial | M | Y | P0 |  |
| ERDAS IMG (HFA) read/write | missing | L | Y | P2 | Esri-origin; GDAL has HFA driver doc |
| ESRI Grid (Arc/Info binary grid, ArcInfo workspace) | missing | L | Y | P2 | ArcGIS default raster format for SA; format reverse engineered by GDAL |
| ASCII grid / Float grid / BIL / BIP / BSQ / ESRI hdr | missing | S | Y | P1 | Esri header; ENVI .hdr overlaps |
| JPEG / PNG / BMP / GIF | missing | S | Y | P2 |  |
| JPEG2000 (.jp2) | missing | L | Y | P3 | decoder (Rust crates exist: jpeg2k, openjpeg) |
| MrSID / ECW | missing | XL | N | P4 | proprietary SDKs; policy: unsupported or optional external lib |
| CRF (Cloud Raster Format) / MRF | missing | L | Y | P3 | MRF open; CRF proprietary |
| Esri geodatabase rasters (raster dataset, raster catalog, mosaic dataset in GDB) | missing | XL | Y | P3 | blob tile storage; bin format |
| NetCDF / HDF4 / HDF5 / GRIB / GRIB2 | missing | L | Y | P2 | NetCDF-4 is HDF5 -> large; classic NetCDF3 simple |
| Multidimensional raster (Zarr, CRF multidim) | missing | L | P | P4 |  |
| LAS/LAZ/zLAS/E57 | missing | L | Y | P1 |  |
| DEM formats: DTED, USGS DEM, SDTS | missing | M | Y | P3 |  |
| NITF / SRTM HGT / SAR (Sentinel SAFE, TerraSAR, Radarsat) , Landsat/Sentinel-2 metadata readers | missing | L | P | P3 | Image Analyst sensor templates; ENVI overlaps |
| Raster function templates (.rft.xml), Raster types | missing | L | Y | P3 |  |

### arcpy API (Python bindings)  (done 0, partial 3, missing 16)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Python bindings (PyO3 `geoforge` module, arcpy-like) | missing | L | Y | P1 | Phase 6; makes Python scripts portable |
| arcpy.da.SearchCursor / UpdateCursor / InsertCursor / Editor | missing | M | Y | P1 | needs gdb edit; shp via existing |
| arcpy.da.Describe, arcpy.Describe, arcpy.ListFields | partial | M | Y | P1 | CLI describe exists |
| arcpy.ListFeatureClasses/ListTables/ListDatasets/ListRasters/ListWorkspaces/Walk | missing | S | Y | P1 |  |
| arcpy.Exists, arcpy.CreateUniqueName, arcpy.ValidateFieldName | missing | S | Y | P1 | rules known (ValidateFieldName in shapefile writer) |
| arcpy.geometry (PointGeometry, Polyline, Polygon, Multipoint; methods: buffer, clip, contains, crosses, disjoint, distanceTo, equals, intersect, overlaps, touches, union, within, ...; projectAs) | partial | L | Y | P1 | geometry engine exists; method surface missing |
| arcpy.Geometry methods: queryPointAndDistance, positionAlongLine, segmentAlongLine, getArea/getLength geodesic | missing | M | Y | P2 |  |
| arcpy.sa (Raster objects, map algebra operators, all SA functions) | missing | L | Y | P1 |  |
| arcpy.na | missing | XL | Y | P3 |  |
| arcpy.mp (Map automation: Project, Map, Layout, Layer, symbology objects, MapSeries, export) | missing | XL | P | P3 | requires renderer; verify via exported PNG, pixel-exact impossible |
| arcpy.metadata, arcpy.management helpers | missing | M | Y | P3 |  |
| arcpy.env / ProcessingEnvironment (workspace, overwriteOutput, extent, cellSize, outputCoordinateSystem, scratchGDB) | missing | M | Y | P0 |  |
| arcpy.AddMessage/AddWarning/AddError/GetMessages/ExecuteError/ProgressBar | missing | S | Y | P1 | messages formats known (WARNING 000304) |
| arcpy.SpatialReference / ListSpatialReferences / ListTransformations | partial | M | Y | P1 | proj crate |
| arcpy.Parameter/ Python toolbox (.pyt) / script tools (.atbx) runner | missing | L | Y | P2 |  |
| arcpy.ImportToolbox / Result objects / geoprocessing history | missing | M | Y | P2 |  |
| arcpy.conversion/analysis/... namespaces & Tool signatures mirrored | missing | M | Y | P1 | auto-gen from tool trait |
| arcpy.ListEnvironments/ ListTools / GetParameterInfo | missing | S | Y | P2 |  |
| arcpy.mapping (legacy) | missing | - | - | P4 | dead |

### Application-level  (done 0, partial 0, missing 20)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Geoprocessing framework (Tool trait, typed params, validation, messages, cancellation, history) | missing | L | Y | P0 | Phase 6; foundation for everything |
| Environment settings framework | missing | M | Y | P0 |  |
| ModelBuilder (.atbx models, iterate, preconditions) | missing | XL | P | P3 | .atbx zip with JSON; model engine on top of Tool trait |
| Python toolboxes (.pyt) / script tools | missing | L | Y | P2 |  |
| Task framework (Tasks .esriTasks) | missing | L | N | P4 | GUI |
| Map view (2D renderer: symbology unique values/graduated/proportional/heat map, labeling Maplex/Standard, transparency, scale ranges) | missing | XL | P | P3 | compare via exported images with tolerance only |
| Layout view / page export (PDF, PNG, TIFF, SVG, JPEG, EPS) | missing | XL | P | P4 |  |
| Scene view (3D, global, local, extrusion, lighting, terrain) | missing | XL | P | P4 |  |
| Symbology (styles .stylx, CIM symbols, marker/line/fill layers, dictionary symbols MIL-STD-2525) | missing | XL | P | P4 | CIM JSON spec public |
| Labeling engine (Maplex) | missing | XL | N | P4 | proprietary placement; not reproducible |
| Attribute table / Selection / Definition queries / Field aliases / Joins in-layer / Charts | missing | L | P | P3 |  |
| Geoprocessing history & metadata of outputs | missing | M | Y | P3 |  |
| Project file (.aprx) read/write (CIM JSON in zip) | missing | L | Y | P2 | CIM schema public (arcgis-pro-sdk) |
| Bookmarks, Time slider, Animation, Share | missing | L | N | P4 |  |
| Coordinate system picker/ Datum transformation UI | missing | M | N | P4 |  |
| Web tools / Publishing (Share As Web Layer, Share Web Tool, Stage Service, Upload Service Definition) | missing | XL | N | P4 | requires portal |
| Portal / ArcGIS Online / ArcGIS Enterprise integration (sign in, content, Feature Services, Hosted layers) | missing | XL | N | P4 | REST API open; feature service client is feasible, but cannot compare to Pro under Wine offline |
| ArcGIS API for Python / REST (Feature service query, Map service export) | missing | L | P | P3 |  |
| Offline: Map packages, Mobile map packages, Offline maps | missing | L | P | P4 |  |
| Plugins/Add-ins/SDK (.NET) | missing | - | N | P4 | skip |

### Utility Network (list only)  (done 0, partial 0, missing 1)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Create Utility Network / Add Rule / Trace / Validate Network Topology / Update Subnetwork / Export Subnetwork | missing | XL | N | P4 | enterprise, branch versioning |

### Location Referencing / Business Analyst / Defense (skipped)  (done 0, partial 0, missing 1)

| Tool / feature | Status | Diff | Verifiable | Prio | Notes |
|---|---|---|---|---|---|
| Location Referencing, Business Analyst, Defense/Military Analyst, Indoors, Parcel Fabric administration | missing | XL | N | P4 | skip per owner |

## (c) Prioritized implementation order

Ranking criteria: user value (frequency of use in ArcGIS workflows), dependency (unblocks others), verifiability (Pro under Wine yields deterministic output), cost.

### Stage 0 - foundations (do before widening)
1. Existing-GDB edit support in `geoforge-filegdb` (rewrite system tables and indexes; add table/feature class/field to an existing GDB; update/delete rows; .atx writing; compaction). Unblocks Add Field/Calculate Field/Join Field/Define Projection/Delete Rows/domain+subtype tools on gdb.
2. Tool trait, parameter model, messages (ERROR/WARNING codes), environments (workspace, overwriteOutput, extent, cellSize, snapRaster, mask, outputCoordinateSystem, scratchGDB), layer/selection model (Make Feature Layer, selections, definition queries).
3. Raster foundation: multi-band model everywhere, environments (cell size/extent/mask/snap), Esri .aux.xml statistics/histograms, pyramids/overviews, COG, ESRI ASCII/BIL/IMG/ESRI Grid readers.
4. Python bindings (PyO3) exposing tools, cursors (arcpy.da), geometry objects and Raster class: allows running arcpy-style scripts and enables direct differential testing against the arcpy bridge using identical scripts.
5. Datum/projection completion (Define Projection, Project Raster, vertical CS, Esri-only methods).

### Stage 1 - recommended next 30 tools (ordered)
1. Define Projection
2. Join Field
3. Create Feature Class / Create Table / Create File GDB / Create Feature Dataset (as commands)
4. Add Fields / Alter Field / Calculate Fields
5. Copy Rows / Delete Rows / Delete Features / Table To Table (Table Select)
6. Get Count / Get Raster Properties / Get Field Value (script helpers)
7. Feature To Polygon
8. Feature To Line / Polygon To Line (known open row-order question; verify what ArcGIS does)
9. Multiple Ring Buffer and Pairwise Buffer/Dissolve (+ geodesic buffer for geographic data)
10. Create Thiessen Polygons (Delaunay + Voronoi)
11. Integrate / Snap / Densify
12. Create Fishnet
13. Raster Clip, Copy Raster, Composite Bands, Extract Bands
14. Mosaic To New Raster / Mosaic
15. Project Raster (nearest, bilinear, cubic, majority)
16. Raster Calculator (map algebra expression parser) with Abs/Exp/Ln/Sqrt/Power/Mod/trig/Boolean/bitwise/comparison ops
17. Cell Statistics, Combine, Pick, Lookup, Weighted Overlay / Weighted Sum
18. Block Statistics, Filter, Extract by Attributes/Points/Rectangle, Extract Values To Points, Sample
19. Zonal Fill / Zonal Geometry / Zonal Histogram / Tabulate Area
20. Euclidean Direction / Allocation + Euclidean Distance with feature sources and max distance
21. Kernel Density / Point Density / Line Density
22. IDW interpolation (SA), then Natural Neighbor, Spline, Trend
23. Cost Distance / Cost Path / Cost Allocation family
24. Mean Center / Median Center / Central Feature / Standard Distance / Directional Distribution
25. Average Nearest Neighbor and Spatial Autocorrelation (Moran's I) with Generate Spatial Weights Matrix
26. Hot Spot Analysis (Gi*) and Cluster and Outlier Analysis (Local Moran)
27. Ordinary Least Squares / Exploratory Regression
28. Table To Excel / Excel To Table (xlsx)
29. Compute Spectral Index (NDVI, etc.) / Band Arithmetic / Principal Components / Maximum Likelihood Classification (deterministic image analysis subset)
30. Convert Coordinate Notation + Create Spatial Reference + Linear Referencing core (Create Routes, Locate Features Along Routes, Make Route Event Layer)

### Stage 2 (next ~60): GDB admin (domains, subtypes, relationship classes, attribute indexes, compact), Table Compare/Feature Compare (also GeoForge's own test tooling), Split Line At Point, Eliminate, Simplify/Smooth Line/Polygon, Cut Fill/Surface Volume, Viewshed exactness, Flow Direction MFD/DINF, Stream To Feature, mobile geodatabase and SpatiaLite, KML/KMZ, DXF, NetCDF3 and GRIB, LAS/LAZ I/O and LAS Dataset To Raster, TIN read/write and TIN To Raster, GWR, DBSCAN, Create Space Time Cube, metadata (.shp.xml, FGDC/ISO), .lyrx/.aprx CIM reading.
### Stage 3: Arcade, topology, Network Analyst (own router first, verified on small networks), geostatistical kriging family, solar radiation, ModelBuilder, 2D renderer, arcpy.mp, mosaic datasets, CRF.
### Stage 4 / policy-bound: deep learning, geocoding, labeling quality, Orthomapping/SfM, enterprise GDB/versioning/Utility Network, Portal publishing, DWG/MrSID/ECW.

## (d) Dependencies

| Item | Needs |
|---|---|
| Any tool writing to existing GDB; domains/subtypes/relationship/attribute index tools; Join Field into gdb | Existing-GDB edit (system tables + .atx/.spx writers) |
| Python bindings, arcpy.da cursors, layers/selections, ModelBuilder, Python toolboxes, history | Tool trait + parameter/message model + environment framework |
| Feature To Polygon, Topology, Integrate, Unsplit, Eliminate, Planarize, Polygon Neighbors | Planar graph / noding engine (shared with Intersect/Union), cluster tolerance support |
| Thiessen, TIN, Natural Neighbor, Topo To Raster, Delaunay-based Spatial Weights | Delaunay triangulation / constrained Delaunay |
| Network Analyst (Route, Service Area, Closest Facility, OD, MLAP, VRP) | Graph model + network dataset reader/writer (.nd, GDB network classes or own format), shortest path (Dijkstra/A*/CH), hierarchy, restrictions, turn tables, time attributes; Service area polygons need alpha shape/trim logic; VRP needs metaheuristics |
| Cost Distance family, Optimal Path, Corridor | Raster graph engine (priority queue over cells with anisotropy, vertical factor) |
| Raster tools with environments | Environment framework (extent, cell size, snap, mask, projection) |
| Project Raster, Warp, Orthorectification | Resampling kernels, raster reprojection grid (shared with proj crate), vertical/datum |
| Mosaic Dataset, Raster Types, Raster functions chain | Raster function graph evaluator, footprints, GDB raster blob storage, statistics, pyramids |
| Image Analyst classification | Multi-band raster stats (covariance), signature files (.gsg/.ecd), training sample handling, ML library (own or linfa) |
| Deep learning | Inference runtime (ONNX Runtime / libtorch via tch / candle), .emd/.dlpk parsing, GPU, tiling/batch pipeline; Wine ArcGIS lacks Python deep-learning env -> cannot verify |
| Space Time Pattern Mining | NetCDF writer, Spatial Statistics (Mann-Kendall, Gi*), spatial weights |
| LAS tools, LAS Dataset To Raster, Classify LAS | LAS/LAZ I/O, spatial index (kd/octree), TIN, rasterization |
| 3D Analyst (Line Of Sight, Skyline) | Surface sampling, TIN/raster visibility, Multipatch generation |
| Geostatistical Analyst | Linear algebra (nalgebra), variogram fitting, neighborhood search, cross validation |
| Linear Referencing | M-aware geometry (done), route measures, event tables, layers |
| Map view / Layout / Symbology / Labeling | Renderer (wgpu), CIM symbol model, text shaping, font handling, tile/pixel pipeline; .lyrx/.aprx parsing |
| Web tools / Portal | REST client, auth, service definition format (.sd/.sddraft), JSON feature service schema |
| Geocoding | Address parsing/standardization rules + reference data + locator format; no stable reference |
| KML/KMZ, GML, DXF, E00, Excel conversion | XML/zip parsing (dependency policy: ok at format-crate level), style translation |
| NetCDF/HDF5/GRIB | An HDF5 reader (large) or accept libhdf5 only as optional backend |
| Spatial Statistics permutation tests | Seeded RNG; ArcGIS's RNG not reproducible - compare with tolerance on distributions |

## (e) ENVI parity

ENVI (NV5 Geospatial) = ENVI + IDL language + ENVI Deep Learning + SARscape + ENVI LiDAR + (formerly) FLAASH, with a Task API (ENVITask, JSON "tasks" run from IDL, Python via `envipy`, or ENVI Services Engine). There is no ENVI install here, so the oracle must be different (see below). ArcGIS Pro Image Analyst already covers a subset.

| ENVI capability area | Overlap with ArcGIS Pro (Image Analyst / SA) | New vs. ArcGIS | Difficulty | Oracle |
|---|---|---|---|---|
| ENVI file format (.hdr + flat binary BSQ/BIL/BIP, also .dat/.img, .sli spectral library, ROI .xml, classification images) | ArcGIS reads ENVI format as a raster type (via GDAL-like driver) | Writer + header metadata (wavelength, fwhm, bbl, band names, map info, coordinate system string, data ignore value, interleave, byte order, header offset) | S | ArcGIS Pro reads/writes ENVI .hdr: verify via bridge. GDAL ENVI driver as test-only oracle |
| Spatial subsetting / spectral subsetting / resize / rotate / flip / layer stacking / mosaicking (seamless, feathering, color balance) | Extract Bands, Clip, Resample, Mosaic, Composite Bands | Spectral subsetting by wavelength; feathering/edge-matching variants; mosaic by georeference pixel-based | S-M | Pro |
| Band math / spectral indices (>80 indices: NDVI, SAVI, EVI, NDWI, NBR, ARVI, hyperspectral indices: PRI, CRI, ARI, WBI, REP...) | Compute Spectral Index, Band Arithmetic | Hyperspectral index catalogue (published formulas) | S | Pro for overlapping indices; formulas from literature otherwise |
| Radiometric calibration (radiance/reflectance, thermal, Landsat/Sentinel/WorldView/Planet sensors) | Compute Apparent Reflectance / Surface Reflectance, Dark Object Subtraction | Sensor-specific calibration from metadata for ~100 sensors | M-L | Pro for the few sensors it supports |
| Atmospheric correction: FLAASH (MODTRAN-based), QUAC, Dark Subtraction, Empirical Line, Flat Field, IARR | none (Pro has only DOS, apparent reflectance) | FLAASH needs radiative transfer (MODTRAN or open 6S/libRadtran substitute); QUAC/FF/IARR/EL are simple statistical | QUAC/FF/IARR/DOS S; FLAASH XL | None for FLAASH; use 6S/libRadtran as independent reference, accept non-identical |
| Classification: supervised (Parallelepiped, Minimum Distance, Mahalanobis, Maximum Likelihood, SVM, Neural Net, KNN, Random Forest, Spectral Angle Mapper, SFF, Spectral Information Divergence, Binary Encoding), unsupervised (ISODATA, K-Means), post-classification (sieve, clump, majority/minority, combine, confusion matrix, ROI-based) | ISO Cluster, MLC, SVM, RF, Classify Raster, Segment Mean Shift, Accuracy Assessment | Many spectral-matching classifiers, ROI workflows, post-classification | M-L | Pro for MLC/ISO/confusion matrix; others from definitions |
| Spectral analysis: Spectral Library Builder/Resampling, Spectral Hourglass (MNF, PPI, n-D visualizer, endmember extraction), Linear spectral unmixing, Matched Filter (MF/MTMF), Constrained Energy Minimization, Adaptive Coherence Estimator, Orthogonal Subspace Projection, Anomaly detection (RX), Target detection, Continuum removal, spectral feature fitting, band-ratio/derivative | SAM only in Pro; PCA | Almost everything | M each | Published algorithms; implement against numpy references (test-only) |
| Dimension reduction: PCA, MNF, ICA, Minimum Noise Fraction | Principal Components | MNF, ICA | M | Pro for PCA only |
| Change detection: image difference/ratio, thematic change, Change Detection Difference Map, Deep learning change | Compute Change Raster, Detect Change Using Change Analysis Raster | Workflow wizards, statistics-based variants | M | Pro |
| Feature extraction (Feature Extraction with Example-Based and Rule-Based): segmentation, attributes, rule classification, export vectors | Segment Mean Shift, Raster To Polygon, Train/Classify | Rule-based object classification with attribute rules; scale-level region merging (Full Lambda Schedule) | L | Pro for segmentation partially; ENVI algorithm not published fully |
| Pansharpening: Gram-Schmidt, HSV, Brovey, Color normalized (CN), NNDiffuse, PC | Pansharpen (Brovey, IHS, Esri, Gram-Schmidt) | CN, NNDiffuse | M | Pro |
| Geometric correction: Image registration, tie points, RPC orthorectification, rigorous orthorectification, DEM extraction, Image Registration Workflow | Georeference, Orthomapping, Warp | Auto tie-point matching, RPC orthorectify | L-XL | Pro for transforms |
| Topographic correction, Image Sharpening, Filters (Convolution, Sobel, Median, Texture co-occurrence, Gabor), Edge detect, Fourier transform (FFT, Filter), Wavelet | Convolution/Filter functions; Focal Statistics | Texture measures (GLCM), FFT, wavelets | M | Pro for convolution/focal |
| Hyperspectral: Spectral subsets by bbl, Hyperspectral Signature Library, N-FINDR, Pixel Purity Index, Smart Pixels, Vegetation Analysis (VI tool), Water vapor retrieval, Atmospheric Correction Module | partly (NDVI only) | All | M-L | Literature |
| LiDAR (ENVI LiDAR, Point cloud): DSM/DEM/DTM, building/tree extraction, point classification | LAS tools, LAS Dataset To Raster, Classify LAS | Feature extraction workflow, tree crown | L | Pro for LAS tools |
| SAR / SARscape (ENVI SARscape): multilooking, speckle filtering (Lee, Frost, Gamma-MAP), geocoding, calibration (sigma0, gamma0), interferometry (InSAR, DInSAR, PSI, SBAS), polarimetry (decomposition: Pauli, Freeman-Durden, H-A-Alpha), SAR classification, SAR change detection | very limited (Image Analyst: SAR utilities (Compute radar...), 'Apply Orbit File'?) | Nearly all of it; heavy | XL | Open SAR toolboxes (ISCE, SNAP) as references; no exact |
| Thermal/ Land surface temperature, Water, Wildfire indices | Compute indices | Few | S-M | Literature |
| Deep learning: ENVI Deep Learning (TensorFlow): Train, Classify Raster Using DL, Object detection | Image Analyst DL tools (Esri .dlpk) | ENVI model format (.h5) | XL | None (see policy) |
| Visualization: Display, Spectral profile viewer, Scatter plot, Z-profile, Animations, Raster color slices, ROI tool, Vector overlay, Annotation | Map view/charts | Spectral profile viewer, n-D visualizer | L | None |
| Task API / IDL: ENVITask (JSON), ENVIRaster/ENVIVectorLayer objects, ENVI Services Engine, `envipy` | arcpy / GP tools | `ENVITask` naming; IDL language | M (task naming layer can be generated from tool trait) | none |
| Interoperability: ENVI .hdr, ENVI Standard, ENVI Classification, ENVI Spectral Library (.sli), ROI (.roi xml), Vector files (EVF), Annotation (.anz) | Pro reads ENVI raster | EVF, SLI, ROI | S | GDAL |
| Metadata-driven sensor readers (Landsat, Sentinel-2, WorldView, SPOT, Pleiades, Planet, MODIS, ASTER, GeoEye, NITF, Radarsat, TerraSAR-X, Sentinel-1) | Raster types | many | L | Pro for those it supports |

Practical plan for ENVI: put the ENVI format and tasks naming layer next to the raster foundation (cheap and useful for hand-off), implement the statistical methods once in a shared "imagery" crate (covariance, PCA/MNF, MLC, SAM, matched filters, unmixing, RX), and cite published algorithm definitions in tests; use ArcGIS Image Analyst as oracle only where its tool is the same algorithm.

## (f) Where exact ArcGIS reproducibility is unlikely or impossible, with policy

| Area | Why | Suggested policy |
|---|---|---|
| Labeling (Maplex/Standard) and Graphic Conflicts / Resolve Road Conflicts / Thin Road Network / Building conflicts | Proprietary combinatorial solvers; results depend on internal heuristics | Implement as "equivalent-function"; test constraints only (no overlap, count within tolerance); document non-parity |
| Symbology rendering pixels, anti-aliasing, fonts, text layout | Rasterizer and font stack differ | Compare structural properties (extent, scale, legend) and image similarity metrics (SSIM threshold), never pixel equality |
| Deep learning tools and ArcGIS pretrained models | Models are Esri-hosted; need GPU; Wine environment cannot run | Support ONNX/PyTorch inference through open runtime; define inputs/outputs per .dlpk spec; verify only shapes/metadata; never claim parity |
| Machine learning with randomness: Random Trees, Forest-based, EBK, K-Means seeds, permutation p-values (Moran, Gi*, Local Moran with permutation), Create Random Points, Subset Features | Different RNG and tree-building implementation | Seedable RNG; test statistical properties (distribution, error bounds); deterministic parts (statistics, z-scores without permutations) compared exactly |
| SVM / Neural nets / ISO Cluster | Solver/initialization dependence | Compare accuracy metrics within tolerance; ISO Cluster compare only when initial means are fixed |
| Geocoding (locators, scoring), online geocode/route/geoenrichment | Reference data and service not available; ArcGIS World services need credentials | Implement local open-data geocoder with own locator format; skip service parity; parse/standardize rules separately documented |
| Network Analyst with ArcGIS Online/Streetmap data, traffic, VRP optimization | Data licensing, heuristics | Match on small custom networks for shortest-path; VRP compare cost bounds not routes |
| Orthomapping / SfM / block adjustment / image matching | Proprietary algorithms | Provide open-source-based equivalent (or external call) outside parity scope |
| Esri proprietary file formats: CRF, mosaic datasets, SDC, ST_Geometry in enterprise, .nd network datasets, locators, terrain datasets, compressed .cdf, zLAS, scene layer packages with Esri extensions, MrSID, ECW | Undocumented/licensed | Black-box analysis only (no decompilation, per CLAUDE.md) on outputs produced by Pro; where licence-bound (MrSID, ECW) support through optional external plugin and document unsupported by default |
| DWG / DGN | Closed formats | Support DXF first; DWG only through documented OpenDesign specs or optional external converter |
| Hillshade/viewshed/flow-direction flats, Contour start vertices, Raster To Polygon SIMPLIFY | Internal heuristics (see roadmap) | Accept bounded differences; keep recorded tolerances in tests (already practice) |
| Interpolators with internal tuning (Topo To Raster, Natural Neighbor, Spline with barriers, EBK, Kernel interpolation) | Algorithm details not fully published | Implement from published math; compare at tolerance (RMSE vs ArcGIS surface) not bit equality |
| Solar Radiation | Algorithm published but numerics (sky sector discretization) sensitive | Match to a few percent |
| Enterprise geodatabase, versioning, Utility Network, Portal/AGOL publishing | Needs servers, licences, accounts; not available under Wine bridge | Out of scope for verification; implement open REST/client layers on demand with mocked servers |
| Task framework, Add-ins, GUI interaction, ModelBuilder UI | GUI | Implement file formats (.atbx, .esriTasks) and headless execution only |
| Date-time/time zone behaviors, Python builtin differences in Calculate Field (arcpy sandbox) | Edge-case semantics | Keep growing the test list (28 expressions today); Arcade and VBScript: Arcade only if requested |
| Floating-point summation order differences (statistics, zonal means) | Order of accumulation | Compare with relative tolerance 1e-9..1e-12 |

Global policy suggestion: (1) classify every tool as Exact (byte/value-level), Tolerant (documented tolerance), Equivalent (same function, different algorithm; tested on properties) or Out-of-scope; record in the tool registry and surface it in `gf describe-tool`. (2) Keep the black-box rule from CLAUDE.md: never decompile; all behaviour from outputs. (3) Add optional-feature crates for licensed or heavy dependencies (ECW/MrSID, ONNX, HDF5) so the core stays dependency-light.
