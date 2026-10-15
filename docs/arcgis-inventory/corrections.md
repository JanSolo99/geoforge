# Corrections to docs/GAP_ANALYSIS.md and docs/gap_list.csv

Checked against the pages fetched for this inventory (doc.esri.com/en/arcgis-pro/latest). "Not found" means: no such tool name in the tool-reference TOC, the toolbox history pages, the ModelBuilder overview, the arcpy reference TOC or the raster-function lists. It does not prove absence from all of ArcGIS. Tool URLs are in the per-toolbox CSVs. Row names refer to the `area` and `tool_or_feature` columns of gap_list.csv.

Method for the mechanical checks: every row of gap_list.csv was split on " / ", "," and "+" into fragments, each fragment was normalised and looked up in the inventory (2,157 tool rows). 275 fragments had no match; the matching ones were compared with the row's toolbox and status. The manual review of those follows.

## A. Overall claims

| Claim in GAP_ANALYSIS.md | Verified |
|---|---|
| "roughly 2,000 geoprocessing tools" | 2,157 tool rows across 45 toolbox CSVs (2,027 listed in the tool-reference TOC across 44 toolboxes, plus 130 found only via toolbox history pages or the ModelBuilder overview; includes 33 deprecated). Several tools appear in two toolboxes (Slope in Spatial Analyst, 3D Analyst; Plus/Minus/Con in Spatial Analyst and Image Analyst), 2,033 distinct arcpy function names. See README.md. |
| "GeoForge has about 80" | 82 `gf` commands; they map to 84 done + 21 partial tool rows (incl. duplicate toolbox rows). |
| "Spatial Analyst (about 250 tools, about 25 done)" | 237 rows (236 in TOC), 24 done, 5 partial. |
| "Image Analyst (about 170 raster functions plus ~60 tools)" | Image Analyst toolbox has 153 tool rows (not ~60). The raster-function list has 187 functions (raster_functions.csv). |
| "Spatial Statistics (~60)" | 61 in TOC (63 incl. 2 deprecated: Grouping Analysis, old Geographically Weighted Regression). |
| "Catalogued items: 469 ... one row may group several tools" | Correct that rows group tools, but the grouping hid ~1,700 tools: 105 of 2,157 tool rows are done or partial (84 + 21), i.e. the done/partial counts per row overstate coverage (a row "Fill / Flow Direction / Flow Accumulation: done" covers three tools of which one is partial). Use the per-tool CSVs. |
| Toolbox coverage | 20 of the 45 toolboxes are not named anywhere in gap_list.csv: AllSource, Aviation, Bathymetry, Crime Analysis and Safety, Data Interoperability, Data Reviewer, GeoAI, GeoAnalytics Desktop, GeoAnalytics Server, Indoor Positioning, Knowledge Graph, Maritime, Network Diagram, Oriented Imagery, Ready To Use, Reality Mapping, Standard Feature Analysis, Territory Design, Topographic Production, Trace Network. Location Referencing, Business Analyst, Defense, Indoors, Parcel and Utility Network appear only inside one summary row each. GeoAI (18 tools), Raster Analysis (29), Multidimension (21), Reality Mapping (14) and ModelBuilder (34) are real toolboxes relevant to the stated goal. |
| "Table To Table", "Feature Class To Feature Class", "Add Geometry Attributes" treated as current tools | They exist but are deprecated: Add Geometry Attributes (replaced by Calculate Geometry Attributes), Feature Class To Feature Class (replaced by Export Features), Table To Table (Conversion; replaced by Export Table), Point Distance (replaced by Near / Generate Near Table). These pages are not in the TOC; they were found via toolbox history pages. See DEPRECATED in the notes column. |

## B. Tools that do not exist under that name (or at all)

| Where | Entry | What the documentation has |
|---|---|---|
| Analysis > Proximity | Generate Near Table 3D | No such tool. `Near 3D` exists, in the 3D Analyst toolbox (3D Proximity toolset). |
| Analysis > Statistics | Statistics (Table Statistics) | Tool is `Summary Statistics` (arcpy.analysis.Statistics). |
| Data Management > Features | Singlepart To Multipart | Not found (page 404). Only Multipart To Singlepart exists; merging is Dissolve. |
| Data Management > Features | Create Features | Not found (404). `Create Feature Class` and `Create Unregistered Feature Class` exist. |
| Data Management > Features | Merge Branch | `Merge Branch` is a ModelBuilder logical tool (merges model branches), not a geometry/feature tool. |
| Data Management > Features | Rotate / Scale / Move / Flip features | Rotate, Flip, Mirror, Shift, Rescale are RASTER tools (Data Management > Projections and Transformations > Raster). There are no vector Rotate/Scale/Move geoprocessing tools; `Transform Features` and `Snap`/`Densify` are in the Editing toolbox. |
| Data Management > Features | Feature Compression | Not found. |
| Data Management > Features | Calculate Geometry Attributes (legacy) | Reversed: `Calculate Geometry Attributes` is the CURRENT tool; `Add Geometry Attributes` (listed as done) is the deprecated one. |
| Data Management > Fields & Tables | Add Fields / Calculate Fields | Real names: `Add Fields (multiple)`, `Calculate Fields (multiple)`, `Alter Fields (multiple)`. |
| Data Management > Fields & Tables | Convert Field | Not found as a tool. `Alter Field`, `Migrate Text Field`, `Convert Temporal Field`, `Encode Field`, `Transform Field` exist. |
| Data Management > Fields & Tables | Add Index / Remove Index | `Add Attribute Index`, `Remove Attribute Index`, `Add Spatial Index`, `Remove Spatial Index`, `Add Full-Text Index` (Indexes toolset). |
| Data Management > Fields & Tables | Generate Rank / Rank by | Not found in Data Management. `Rank` is a Spatial Analyst Local tool. |
| Data Management > Fields & Tables | Generate Unique ID / GlobalIDs (Add GlobalIDs) | `Add Global IDs`, `Add Incrementing ID Field`, `Generate ID Attribute Rule`. |
| Data Management > Fields & Tables | Summarize Attributes / Concatenate / Cross Tabulate | `Summarize Attributes` exists only in GeoAnalytics (Desktop/Server). `Concatenate` and `Cross Tabulate` not found. Real DM tools: `Pivot Table`, `Transpose Fields`, `Field Statistics To Table`. |
| Data Management > Fields & Tables | Text-processing: Parse Path, Create Unique Name, Validate Table Name | `Parse Path` is a ModelBuilder tool; the others are arcpy functions (arcpy.CreateUniqueName, arcpy.ValidateTableName), not tools. |
| Data Management > Fields & Tables | Field Mapping / Field Info | arcpy classes (FieldMappings, FieldInfo), not tools. |
| Data Management > Fields & Tables | Table To Table (as current) | Conversion toolbox, deprecated; replaced by `Export Table`. |
| Data Management > Workspace | Upgrade Spatial Reference | Not found. `Upgrade Dataset`, `Upgrade Geodatabase` exist; "Detect and repair invalid spatial references" is a concept page. |
| Data Management > Workspace | Create Enterprise Geodatabase / Mobile GDB | `Create Enterprise Geodatabase`, `Enable Enterprise Geodatabase`, `Create Mobile Geodatabase`, `Create SQLite Database` exist (separate tools). |
| Data Management > Workspace | Compact / Compress File GDB | `Compact`, `Compress File Geodatabase Data`, `Uncompress File Geodatabase Data` (File Geodatabase toolset). `Compress` is a different tool (Geodatabase Administration, enterprise versioned data). |
| Data Management > Workspace | Create/Manage Versions, Reconcile, Post | `Create Version`, `Alter Version`, `Change Version`, `Delete Version`, `Reconcile Versions` (post is a parameter of Reconcile Versions). "Manage Versions" is not a tool. |
| Data Management > Workspace | Geometric Network / Create Geometric Network | No creation tool in the tool reference. "Supported data types" lists geometric networks as display only (Create: No). |
| Data Management > Workspace | Parcel Fabric tools | Separate Parcel toolbox (25 tools); `Build Parcel Fabrics` / `Validate Parcel Fabric` appear in history only. |
| Data Management > Projections | Rectify / Georeference | No such tools. Available: `Warp`, `Warp From File`, `Register Raster`, `Project Raster`, `Define Projection` (also for rasters; no separate "Raster Define Projection"). |
| Data Management > Projections | Reproject on-the-fly (map) | Application behaviour, not a tool. |
| Data Management > Raster | Extract Bands | Raster function only (also `Subset Bands`). `Composite Bands` is the tool. |
| Data Management > Raster | Cell Statistics / Band Collection Statistics | Spatial Analyst (Local / Multivariate), also Image Analyst for Cell Statistics. |
| Data Management > Raster | Pansharpen (DM) | Tools: `Create Pansharpened Raster Dataset`, `Compute Pansharpen Weights`. `Pansharpen` is a raster function. |
| Data Management > Raster | Shaded Relief | Raster function (Surface category) only. |
| Data Management > Raster | Create Raster Catalog | Not found (raster catalogs migrate to mosaic datasets per the supported-data page). |
| Data Management > Raster | Compute Pansharpen Weights, Color Balance | `Color Balance Mosaic Dataset` is the tool name. |
| Data Management > Raster | Convert Raster Format to CRF | `Copy Raster` (and `Raster To Other Format` in Conversion) writes CRF; no dedicated tool. |
| Data Management > Raster | Mask / Nodata handling environments | `Mask` is an environment; `Extract by Mask` is the SA tool; `Mask` is also a raster function. |
| Data Management > Layers | Calculate Value / Iterate Features (ModelBuilder) | `Calculate Value` is a ModelBuilder toolbox tool. `Iterate Features` does not exist; real iterators: `Iterate Feature Selection`, `Iterate Feature Classes`, `Iterate Row Selection`, ... |
| Data Management > Layers | Get Field Value | ModelBuilder toolbox (`Get Field Value`). `Get Count`, `Get Raster Properties`, `Get Cell Value` are Data Management. |
| Data Management > Generalization | (whole row set) | See section C: most of these tools are Cartography. In Data Management the Generalization toolset contains only `Dissolve`, `Eliminate`, `Eliminate Polygon Part`. `Dissolve Boundaries` is GeoAnalytics / Standard Feature Analysis. |
| Data Management > LAS | LAS reader/writer, Classify LAS ..., Convert LAS, Tile/Thin LAS, Locate Outliers, Colorize LAS | Not in Data Management: `Classify LAS *`, `Thin LAS`, `Tile LAS`, `Extract LAS`, `Colorize LAS`, `LAS To Multipoint` are 3D Analyst (Point Cloud); `Convert LAS`, `LAS Dataset To Raster`, `Point Cloud To Raster` are Conversion; `Locate Outliers` is 3D Analyst Triangulated Surface (TIN), not LAS. DM LAS Dataset toolset has 7 tools (Create LAS Dataset, Add/Remove Files, Build LAS Dataset Pyramid, LAS Dataset Statistics, LAS Point Statistics As Raster, Project LAS). |
| Conversion | Points To Raster | `Point to Raster`. |
| Conversion | Export Raster Data / Raster To Video | Not found. `Raster To Other Format`, `Copy Raster`. |
| Conversion | Import CAD | `CAD To Geodatabase`. |
| Conversion | GML / WFS To Features / OGC (WMS, WCS) | Only `WFS To Feature Class`; no GML or WMS/WCS conversion tool. WMS/WCS are service connections. |
| Conversion | Import XML / Export XML Workspace Document | Data Management > Workspace: `Import XML Workspace Document`, `Export XML Workspace Document`. |
| Conversion | Metadata tools (Import Metadata, Export Metadata, Upgrade Metadata, FGDC/ISO 19139) | No metadata geoprocessing tools found in the 3.x tool reference; metadata is the `arcpy.metadata` module (arcpy_api.csv). |
| Conversion | Multidimensional: NetCDF To Raster/Feature/Table, Make NetCDF Layers, HDF, GRIB | Multidimension toolbox: `Make NetCDF Raster Layer`, `Make NetCDF Feature Layer`, `Make NetCDF Table View`, `Raster to NetCDF`, `Feature to NetCDF`, `Table to NetCDF`, `NetCDF ... (Discrete Sampling Geometry)` tools. HDF and GRIB are raster formats, not tools. |
| Conversion | Feature Class To CAD / Export CAD | `Export To CAD`. |
| Conversion | Excel / Parquet / Arrow | `Excel To Table`, `Table To Excel`, `Export To Parquet`. No Arrow tool found. |
| Conversion | Table To Table to SQLite/Database | Not found; use `Export Table`. |
| Conversion | Raster To LAS and LAS To Raster | `Mesh To LAS`, `LAS Dataset To Raster`, `Point Cloud To Raster`. No `Raster To LAS`. |
| Conversion | OSM / Load OSM | Not found. |
| Conversion | ASCII To Raster / Float To Raster | Exist but deprecated (found via history). `Raster to ASCII` and `Raster to Float` are current. |
| Cartography | Grids and Graticules (Grid, Graticule, Measured Grid, Reference Grid) | No such Cartography toolset. Cartography has Map Series (`Grid Index Features`, `Strip Map Index Features`); `Make Grids And Graticules Layer` is Topographic Production. |
| Cartography | Annotation (Feature To Annotation) | `Feature To Annotation` not found. Annotation toolset: `Annotate Selected Features`, `Contour Annotation`, `Convert Labels To Annotation`, `Convert Labels To Graphics`, `Tiled Labels To Annotation`, `Update Annotation Reference Scale`, `Map Server Cache Tiling Scheme To Polygons`. |
| Cartography | Cartographic Partitions, Data Driven Pages (Build Cartographic Partition) | `Create Cartographic Partitions` (Generalization toolset). Data driven pages are a layout feature. |
| Cartography | Symbology tools (Apply Symbology From Layer, Add Colormap, ...) | `Apply Symbology From Layer` and `Match Layer Symbology To A Style` are Data Management (Layers); `Add Colormap` is Data Management Raster Properties. |
| Cartography | Representations / Cartographic Representation tools | No representation toolset in the current Cartography TOC. |
| Cartography | Tiling schemes: Generate Map Server Cache Tiling Scheme; Create Map Tile Package; Create Vector Tile Package | Server toolbox / Data Management Package toolset, not Cartography. |
| Spatial Analyst | Sqrt, Ceil/Floor, And/Or/Not/Xor | `Square Root`; `Round Up`/`Round Down`; `Boolean And/Or/Not/XOr`, `Combinatorial And/Or/XOr`, `Bitwise ...`. |
| Spatial Analyst | Majority / Minority / Variety | Statistic types of Focal/Block/Cell Statistics, not tools. `Majority Filter` is a tool. |
| Spatial Analyst | Viewshed 2 | Renamed `Geodesic Viewshed` (SA Surface and 3D Analyst Visibility). |
| Spatial Analyst | Area and Volume Statistics | Not found. `Cut Fill` (SA, 3D), `Surface Volume`/`Polygon Volume`/`Surface Difference` (3D Analyst), `Calculate Cut Fill Volume` (Image Analyst). |
| Spatial Analyst | Region Network | `Optimal Region Connections`. |
| Spatial Analyst | Kernel Density with barriers | Not found. Barriers exist in `Kernel Interpolation With Barriers` (Geostatistical Analyst). |
| Spatial Analyst | Geodesic Slope/Aspect | No separate tools; Slope/Aspect have a geodesic method option (as in the Surface section of the page). |
| Spatial Analyst | Thin Raster; Sunmap | `Thin`; `Solar Radiation Graphics`. |
| Spatial Analyst | Stream Order (Shreve) | Shreve is a method option of `Stream Order`. |
| Spatial Analyst | Environment settings | Not tools (see tool-reference environment-settings pages). |
| Spatial Analyst | Raster Creation / Reclass etc. listed as "Surface/Hydrology/Distance/Density" rows | Reclass, Local, Raster Creation, Overlay, Generalization, Multivariate, Segmentation and Classification are separate SA toolsets; Fuzzy Membership/Fuzzy Overlay/Weighted Overlay/Weighted Sum/Multicriteria Overlay/Locate Regions are SA Overlay. |
| 3D Analyst | TIN Create | `Create TIN`. |
| 3D Analyst | Surface Information | `Add Surface Information` (SA and 3D). |
| 3D Analyst | Features To 3D | `Feature To 3D By Attribute`. |
| 3D Analyst | Sky Barrier / Shadow Volume | `Skyline Barrier`; `Sun Shadow Volume`, `Sun Shadow Frequency`. |
| 3D Analyst | Viewshed 3D | Not found as a tool (Viewshed, Geodesic Viewshed, Intervisibility, Line Of Sight). |
| 3D Analyst | Voxel tools | Multidimension toolbox: `Make Multidimensional Voxel Layer`, `Export Voxel Isosurface`. |
| Geostatistical Analyst | Exploratory Spatial Data Analysis (histogram, Voronoi map, QQ plot, semivariogram cloud) | Wizard/chart functionality; GP tools: `Exploratory Interpolation`, `Semivariogram Sensitivity`, `Cross Validation`, `Directional Trend`. |
| Geostatistical Analyst | IDW, Global/Local Polynomial, RBF, Diffusion/Areal interpolation | Real names: `IDW`, `Global Polynomial Interpolation`, `Local Polynomial Interpolation`, `Radial Basis Functions`, `Diffusion Interpolation With Barriers`, `Areal Interpolation Layer To Polygons`; also `Empirical Bayesian Kriging`, `EBK Regression Prediction`, `Empirical Bayesian Kriging 3D`, `Moving Window Kriging`, `Kernel Interpolation With Barriers`, `IDW 3D`, `Nearest Neighbor 3D` (missing from the gap list: 12 interpolation tools in the TOC). |
| Geostatistical Analyst | Cross Validation / Validation | `Validation` not found; `Cross Validation` exists. |
| Network Analyst | Make Route/Closest Facility/Service Area/OD/Location-Allocation/VRP layer | Current names are `Make Route Analysis Layer`, `Make Closest Facility Analysis Layer`, `Make Service Area Analysis Layer`, `Make OD Cost Matrix Analysis Layer`, `Make Location-Allocation Analysis Layer`, `Make Vehicle Routing Problem Analysis Layer`, plus `Make Last Mile Delivery Analysis Layer`, `Make Waste Collection Analysis Layer`; the short names (Make Route Layer ...) are deprecated. |
| Network Analyst | Network Analysis arcpy.na module | The current module is `arcpy.nax`; `arcpy.na` is listed under "Legacy" in the arcpy TOC. |
| Network Analyst | Utility Network (Trace, Validate, ...) | Utility Network (122 rows) and Trace Network (13) are separate toolboxes, not Network Analyst. |
| Network Analyst | Network Analyst on public transit (GTFS) | Public Transit toolbox plus Conversion GTFS tools (history-listed: `GTFS Stops To Features`, `GTFS To Network Dataset Transit Sources`, ...). |
| Spatial Statistics | Create Spatial Weights Matrix | `Generate Spatial Weights Matrix` (and `Generate Network Spatial Weights`). |
| Spatial Statistics | Grouping Analysis | Deprecated; replaced by `Multivariate Clustering` / `Spatially Constrained Multivariate Clustering`. |
| Spatial Statistics | Geographically Weighted Regression, Multiscale GWR | Current names: `Geographically Weighted Regression (GWR)`, `Multiscale Geographically Weighted Regression (MGWR)`; the plain `Geographically Weighted Regression` is deprecated. |
| Spatial Statistics | Spatial Lag/Error, Spatial Regression | `Spatial Autoregression`, `Generalized Linear Regression`. |
| Spatial Statistics | Kernel Interpolation With Barriers / Areal Interpolation | Geostatistical Analyst, not Spatial Statistics. |
| Spatial Statistics | Subset Features | Data Management and Geostatistical Analyst. |
| Spatial Statistics | Trend Detection; Geographic Cluster Detection (SaTScan-like) | Not found. (Mann-Kendall is part of Emerging Hot Spot Analysis in Space Time Pattern Mining.) |
| Spatial Statistics | Calculate Distance Band | `Calculate Distance Band from Neighbor Count`. |
| Spatial Statistics | Standard Distance etc. | Exist as listed. Missing from the gap list: `Average Nearest Neighbor` ok; `Linear Directional Mean`, `Neighborhood Summary Statistics`, `Time Series Smoothing`, `Dimension Reduction`, `Estimate Time to Event`, `Causal Inference Analysis`, `Presence-only Prediction (MaxEnt)`, `Prepare Data for Prediction`, `Evaluate Predictions with Cross-validation`, Moran-eigenvector tools, `Spatial Outlier Detection`, `Similarity Search`, `Colocation Analysis`. |
| Space Time Pattern Mining | Create Space Time Cube | Three tools: `...By Aggregating Points`, `...From Defined Locations`, `...From Multidimensional Raster Layer`. |
| Space Time Pattern Mining | Visualize Space Time Cube | `Visualize Space Time Cube in 2D`, `... in 3D`, `Make Space Time Cube Layer` (history also lists "Visualize the Space Time Cube in 2D/3D", pages not found). |
| Space Time Pattern Mining | Exponential Smoothing; Mann-Kendall trend | `Exponential Smoothing Forecast`; Mann-Kendall is not a tool. STPM also has `Change Point Detection`, `Time Series Cross Correlation`, `Fill Missing Values`, `Evaluate Forecasts By Location`, `Subset Space Time Cube` not in the list. |
| Image Analyst | Compute Spectral Index; Compute Surface Reflectance; Dark Object Subtraction | Not found in the tool reference or raster-function list. (`Apparent Reflectance`, `Band Arithmetic`, `NDVI` are raster functions.) |
| Image Analyst | Maximum Likelihood, ISO Cluster, Principal Components (as Image Analyst) | Those are Spatial Analyst Multivariate tools; Image Analyst has the Train * classifier tools. |
| Image Analyst | Change Detection Wizard | GUI, not a tool. Tools: `Compute Change Raster`, `Detect Change Using Change Analysis Raster`, `Analyze Changes Using CCDC`, `Analyze Changes Using LandTrendr`. |
| Image Analyst | Super Resolution, Object tracking, Fill Voids, Remove Cloud, Thematic | Not found as tools. |
| Image Analyst | Orthomapping / Compute Camera Model / Build Footprints | Data Management (Ortho Mapping) and Reality Mapping toolbox; `Build Footprints` is a mosaic dataset tool. |
| Image Analyst | Raster To Polygon | Conversion. |
| Geocoding | Standardize Addresses / Geocode Table | `Standardize Addresses` and `Create Address Locator` are deprecated; `Geocode Addresses` geocodes tables. Missing from the list: `Create Feature Locator`, `Clip Locator`, `Rematch Addresses`, `Split Address Into Components`, `Assign Streets To Points`, `Batch Geocode` (portal). |
| Linear Referencing | Route Event Properties; Fabricate Routes; Create Route Event Tables | Not tools. Real toolbox (7 tools): `Calibrate Routes`, `Create Routes`, `Dissolve Route Events`, `Locate Features Along Routes`, `Make Route Event Layer`, `Overlay Route Events`, `Transform Route Events`. Location Referencing (LRS) is its own 49-tool toolbox. |
| Editing | Append / Merge / Divide / Cut / Union / Planarize / Reshape / Fillet | Editor map tools, not geoprocessing tools. Editing toolbox (19): `Densify`, `Erase Point`, `Extend Line`, `Flip Line`, `Generalize`, `Simplify By Straight Lines And Circular Arcs`, `Simplify By Tangent Segments`, `Snap`, `Trim Line`, `Update COGO`, and the Conflation toolset. |

## C. Right tool, wrong toolbox/area in gap_list.csv

| Entry | Row area | Actual toolbox / toolset |
|---|---|---|
| Select Layer By Attribute | Analysis > Extract | Data Management > Layers and Table Views |
| Feature To Polygon, Feature To Line, Polygon To Line, Split Line At Point, Split Line At Vertices | Analysis > Overlay | Data Management > Features |
| Interpolate Points, Join Features, Aggregate Points, Create Buffers | Analysis > Overlay/Proximity | Raster Analysis, GeoAnalytics Desktop/Server, Standard Feature Analysis (portal/server); `Aggregate Points` also Cartography |
| Pairwise Clip/Intersect/Erase/Dissolve | Analysis > Proximity | Analysis > Pairwise Overlay toolset (also Pairwise Buffer, Pairwise Integrate, Pairwise Polygon Neighbors) |
| Near 3D | Analysis > Proximity | 3D Analyst > 3D Proximity |
| Tabulate Area | Analysis > Statistics | Spatial Analyst > Zonal |
| Feature Class To Feature Class, Feature Class To Geodatabase, Table To Table, Table To Geodatabase | Data Management | Conversion (the first and Table To Table are deprecated) |
| XY Table To Point, Export XML Workspace Document | Conversion | Data Management |
| Snap, Densify, Transform Features | Data Management > Features | Editing |
| Feature To 3D By Attribute | Data Management > Features | 3D Analyst |
| Clip (raster use) | Data Management > Raster | `Clip Raster` (DM); `Clip` is Analysis (vector) |
| Cell Statistics, Band Collection Statistics | Data Management > Raster | Spatial Analyst |
| Raster To Other Format | Data Management > Raster | Conversion |
| Simplify Line/Polygon, Smooth Line/Polygon, Thin Road Network, Aggregate Points/Polygons, Delineate Built-Up Areas, Collapse Dual Lines, Collapse Road Detail (and Resolve Building Conflicts) | Data Management > Generalization & Simplification | Cartography > Generalization (Resolve Building Conflicts: Cartography > Graphic Conflicts) |
| Surface Volume | Spatial Analyst | 3D Analyst (Area and Volume) |
| Rescale | Spatial Analyst | Data Management > Raster (a raster geometry tool; `Rescale by Function` is Spatial Analyst Reclass) |
| Contour / Kriging (as 3D/GA) | Geostatistical Analyst | Spatial Analyst / 3D Analyst |
| Kernel Interpolation With Barriers, Subset Features | Spatial Statistics | Geostatistical Analyst |
| ISO Cluster, Principal Components | Image Analyst | Spatial Analyst Multivariate |
| Create Space Time Cube | Space Time Pattern Mining | exists in STPM (3 variants); GeoAnalytics Server also has a cube tool |
| Contour List / Contour With Barriers | listed as missing in SA | both exist in SA Surface (and `Contour List` in 3D) |
| Make Grids And Graticules Layer (implied) | Cartography | Topographic Production |
| Geocoding rows for tools under "Portal toolset" | Geocoding | `Batch Geocode`, `Geocode File`, `Geocode Locations From Table` are portal (online) tools |

## D. Status errors (stale or wrong relative to the repository)

Compared against `gf --help` and docs/ROADMAP.md (as read at the start of this task), using the same status rule as the CSVs.

| Entry | gap_list.csv | Should be |
|---|---|---|
| Split | partial | done (ROADMAP: Split matches ArcGIS) |
| Update | partial | done (`gf update`) |
| Append | partial | done (with field mapping) |
| Add Spatial Index | partial | done (shapefile) |
| Near, Generate Near Table | done | partial (planar only, no geodesic) |
| Minimum Bounding Geometry | done | partial (options not all verified) |
| Check Geometry, Repair Geometry | done | partial (ROADMAP: GDB output tables, curves, multipoint, Z/M open) |
| Project | done | partial (ROADMAP: Esri-only methods, grids beyond NADCON open) |
| Resample | done | partial (square cells only) |
| Export Table | done | partial (to .csv/.gpkg only) |
| Flow Direction, Flow Length | done | partial (flats approximated; unweighted only) |
| Stream Order | missing ("Stream To Feature / Stream Order (Shreve)") | done (`gf stream-order`) |
| Reclassify | partial | done |
| Create Folder, Create Feature Dataset | partial | missing (ROADMAP: feature datasets in writer not done; no gf command) |
| Select Layer By Attribute | partial | missing (gf `select` implements Analysis > Select, a different tool) |
| Feature Class To Shapefile, Table To dBASE | partial | missing as commands (gf `copy-features` writes shapefiles but is a different tool) |
| Raster To Other Format | partial | missing |
| Slice, Lookup | partial | missing |

GAP_ANALYSIS.md section (b)/(c) tables inherit these. Its own status note says marks predate Sort, Delete Identical, Add/Delete Field, Calculate Field, Update, Symmetrical Difference.

## E. Other errors in prose sections

- (c) Stage 1 item 5 "Table To Table (Table Select)": two different tools; Table To Table is deprecated (Export Table replaces it); `Table Select` (Analysis, Extract) is current.
- (c) Stage 1 item 6 "Get Field Value (script helpers)": ModelBuilder tool; Get Count / Get Raster Properties are DM tools.
- (c) Stage 1 item 13 "Extract Bands": raster function only, not a tool; (c) item 29 "Compute Spectral Index": not found.
- (c) Stage 1 item 1: Define Projection etc. fine; item 11 "Integrate / Snap / Densify": Integrate is DM; Snap and Densify are Editing.
- (c) Stage 3 "ModelBuilder": there is a ModelBuilder toolbox of 34 tools (iterators, logic, utilities); several need an expression evaluator (Python or Arcade).
- (e) "Apply Orbit File": the Image Analyst tools are `Apply Orbit Correction` and `Download Orbit File`. "Compute Apparent Reflectance / Surface Reflectance": `Apparent Reflectance` is a raster function; no Surface Reflectance found. "SAM only in Pro": not verified.
- (e) "Image Analyst: SAR utilities (Compute radar...)": the SAR toolset has 24 named tools (Apply Radiometric Calibration, Apply Geometric Terrain Correction, Generate Interferogram, Unwrap Phase, Despeckle, ...). This is far more than "very limited" and is itself a HARD item.
- (f) "Task framework, Add-ins": no such toolbox; out of scope remark is fine.
- (d) "Geometric network" etc.: Pro displays but cannot create geometric networks.
- File formats (vector): "Mobile Geodatabase read/write", "KML/KMZ", "CAD", "Excel": the supported-data-types page says KML/KMZ, CAD (.dgn/.dwg/.dxf), Excel (.xls/.xlsx) and text tables are read-only (Create: No, Edit: No). "Personal geodatabase (.mdb)", "Coverage/E00", "GML", "TopoJSON", "FlatGeobuf", "SpatiaLite", "MBTiles", "LAZ", "E57" do not appear on the "Supported data types and items" or "Raster file formats" pages at all (only `Convert Personal Geodatabase` as a tool, and `Convert LAS`); the claim that ArcGIS supports them is unverified. SQLite database (.sqlite) and GeoPackage (.gpkg) are listed.
- File formats (raster): "ASCII grid / Float grid / BIL / BIP / BSQ / ESRI hdr" are listed (ASCII Grid read-only, write needs Spatial Analyst); "NITF / SRTM HGT ..." listed; "MrSID" read-only, "ECW" read-only and optional install component per the page. The raster formats page has 70+ formats; the gap list covers about 25.
- arcpy API: the list omits modules that the arcpy reference TOC has: Charts, Geocoding (`arcpy.geocoding`), Image Analysis (`arcpy.ia`, 160 pages), Network Analyst (`arcpy.nax`, 145 pages), Sharing, Metadata, Utility Network, `arcpy.da` (30 pages: 17 functions + 12 classes), Mapping (84 pages). Names such as "arcpy.na" are legacy; the SpatialReference/Geometry rows refer to classes that exist.

## F. Missing entirely from the gap analysis (existing in ArcGIS Pro, found in the inventory)

Counts from the per-toolbox CSVs, tools the gap list never mentions by name: Data Management has 441 tool rows; Spatial Analyst 237; 3D Analyst 145 (TIN/terrain/LAS/visibility/point-cloud deep learning); Image Analyst 153 (SAR 24, deep learning 14, multidimensional 9, change detection 4, spectral analysis 7, motion imagery 5); GeoAI 18; Raster Analysis 29; Multidimension 21; Reality Mapping 14; ModelBuilder 34; Server 19; and the toolboxes listed in section A. Whole families missing from the gap list: Data Management Mosaic Dataset (32 tools), Ortho Mapping (17), Distributed Geodatabase/replicas (13), Versions (10), Attribute Rules (14 incl. templates), Contingent Values (8), Data Comparison (6), Packaging (27 incl. 9 scene-layer tools), Tile Cache (4), Trajectory (3), Catalog Dataset (3), Feature Binning (3), Spectral Library (1), Reports (2), Photos (2); Spatial Analyst Groundwater (4), Solar Radiation (5), Overlay (6), Segmentation and Classification (16); Cartography Cartogram + Cartographic Refinement (13)/Masking (3)/Map Series (6).
