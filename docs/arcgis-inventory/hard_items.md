# HARD items: what is documented about the algorithm, and what is not

Every algorithm summary below was taken from a page on doc.esri.com that was fetched for this inventory; the URL follows each item. Where an item says "nothing summarised", no algorithm page was read for it. Words such as "undocumented" or "proprietary" in the Why lines are the inventory author's judgement from the pages read; they are not an exhaustive check of all Esri documentation. Difficulty and exactness are judgements by the inventory author, not statements by Esri.

Legend: difficulty S/M/L/XL; exact = is exact (ArcGIS-identical) reproduction plausible: Y yes, M maybe (needs ArcGIS comparison), N no/unlikely. HARD (Y) = exact algorithm unknown, heavy numerics, proprietary format/solver, or ML. DOC = algorithm is documented, work is moderate; listed because it was on the "look for" list or is easy to under-estimate.

Counts: 55 HARD groups covering 664 distinct (toolbox, tool) rows; 13 DOC groups; 12 non-tool HARD areas. Online-service-dependent tools found by a text rule are flagged in the CSVs (hard_reason starting "Online/portal").

## Part 1. HARD groups (tools)

### 3D Analyst: Classify LAS Ground, Classify LAS Building, Classify LAS Noise, Classify LAS Overlap, Classify LAS By Height, Extract LOD2 Buildings ...
- Tools covered (27): Buffer 3D; Classify LAS Building; Classify LAS By Height; Classify LAS Ground; Classify LAS Noise; Classify LAS Overlap; Difference 3D; Enclose Multipatch; Enforce River Monotonicity; Extract LOD2 Buildings; Extract Mesh Features Using Point Cloud; Extract Multipatch From Mesh; Extract Objects From Point Cloud; Extract Power Lines From Point Cloud; Extract Rails From Point Cloud; Extrude Between; Features From CityEngine Rules; Generate Clearance Surface; Import 3D Files; Inside 3D; Intersect 3D; LAS Building Multipatch; Minimum Bounding Volume; Near 3D; Regularize Adjacent Building Footprint; Regularize Building Footprint; Union 3D
- Difficulty XL; exact reproduction: N
- Why: Point cloud classification/feature extraction and 3D geometry algorithms are undocumented heuristics (ground filter, building detection); LAS/LAZ I/O is separate work.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/classify-las-ground.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/buffer-3d.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/classify-las-building.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/classify-las-by-height.html ...

### 3D Analyst: Create TIN, Edit TIN, Copy TIN, Decimate TIN Nodes, Delineate TIN Data Area, TIN To Raster ...
- Tools covered (23): Copy TIN; Create TIN; Decimate TIN Nodes; Delineate TIN Data Area; Edit TIN; LAS Dataset To TIN; LandXML To TIN; Locate Outliers; Polygon Volume; Raster To TIN; Surface Aspect; Surface Contour; Surface Difference; Surface Slope; Surface Volume; TIN Domain; TIN Edge; TIN Line; TIN Node; TIN Polygon Tag; TIN To Multipatch; TIN To Raster; TIN Triangle
- Difficulty XL; exact reproduction: M
- Why: TIN dataset is a proprietary binary format; constrained Delaunay with breaklines/clip/erase is documented as concepts only.
- Documented: Create TIN: Delaunay triangulation rule (avoid geographic coordinates); point features as mass points, lines as mass points and/or breaklines; polygons as clip, erase, replace etc.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/create-tin.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/how-create-tin-3d-analyst-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/how-tin-to-raster-3d-analyst-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/copy-tin.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/create-tin.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/decimate-tin-nodes.html ...

### 3D Analyst: Create Terrain, Build Terrain, Add Feature Class To Terrain, Add Terrain Pyramid Level, Remove Terrain Pyramid Level, Append Terrain Points ...
- Tools covered (15): Add Feature Class To Terrain; Add Terrain Pyramid Level; Append Terrain Points; Build Terrain; Change Terrain Reference Scale; Change Terrain Resolution Bounds; Create Terrain; Create Terrain From BIM; Delete Terrain Points; Remove Feature Class From Terrain; Remove Terrain Pyramid Level; Replace Terrain Points; Terrain To Points; Terrain To Raster; Terrain To TIN
- Difficulty XL; exact reproduction: N
- Why: Terrain datasets (pyramids, window-size thinning, geodatabase storage) are proprietary multi-resolution TINs.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/how-terrain-pyramid-level-definition-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/add-feature-class-to-terrain.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/add-terrain-pyramid-level.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/append-terrain-points.html ...

### 3D Analyst, Spatial Analyst: IDW, Natural Neighbor, Spline, Spline with Barriers, Trend, Kriging ...
- Tools covered (16): IDW; Kriging; Natural Neighbor; Spline; Spline with Barriers; Topo to Raster; Topo to Raster by File; Trend
- Difficulty XL; exact reproduction: M
- Why: Interpolators have documented maths but search-neighbourhood, variogram fitting and solver tolerances determine results; Topo to Raster is ANUDEM 5.3 (external published program, iterative drainage enforcement) and Spline with Barriers is not described at all.
- Documented: IDW: linearly weighted combination of samples with inverse-distance power (default 2) and search neighbourhood. Natural Neighbor: Sibson area-stealing weights from Voronoi. Spline: minimum-curvature thin-plate (regularized or tension) passing through points. Trend: global polynomial up to order 12. Kriging: geostatistical model of spatial autocorrelation with semivariogram fit. Topo to Raster: ANUDEM (Hutchinson) version 5.3 with drainage enforcement and contour-aware interpolation.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-idw-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-natural-neighbor-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-spline-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-trend-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-kriging-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-topo-to-raster-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-spline-with-barriers-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/idw.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/kriging.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/natural-neighbor.html ...

### 3D Analyst, Spatial Analyst: Viewshed, Geodesic Viewshed, Observer Points, Visibility, Skyline, Skyline Barrier ...
- Tools covered (16): Construct Sight Lines; Geodesic Viewshed; Intervisibility; Line Of Sight; Observer Points; Skyline; Skyline Barrier; Skyline Graph; Sun Shadow Frequency; Sun Shadow Volume; Viewshed; Visibility
- Difficulty XL; exact reproduction: M
- Why: Classic Viewshed uses an undocumented wavefront-like algorithm (ROADMAP: only approximated); Geodesic Viewshed is documented as independent geocentric 3D sight lines with optional GPU and vertical-error handling.
- Documented: Geodesic Viewshed: transforms the elevation surface to a geocentric 3D system and tests a 3D sight line from each observer to each transformed cell centre independently (no wavefront), optionally with vertical uncertainty and an observer-region table for up to 32 observers.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-viewshed-2-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/using-viewshed-and-observer-points-for-visibility.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/construct-sight-lines.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/intervisibility.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/line-of-sight.html ...

### Analysis: Buffer
- Tools covered (1): Buffer
- Difficulty L; exact reproduction: M
- Why: Geodesic method and one-sided (side type) dissolve of line buffers are only described qualitatively; arcs/offset construction details undocumented.
- Documented: Routine traverses each input vertex, creates buffer offsets, and builds output from the offsets. Method PLANAR picks Euclidean for projected input and geodesic for geographic input with linear distance; GEODESIC densifies the input and builds shape-preserving geodesic buffers. For DISSOLVE-ALL with a side type, input lines are first connected into a pseudo network, the longest connected runs are buffered on one side; non-dissolved one-sided buffers are made per line.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/how-buffer-analysis-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/buffer.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/buffer.html

### Analysis: Enrich
- Tools covered (1): Enrich
- Difficulty XL; exact reproduction: N
- Why: Needs Esri demographic/landscape data service (ArcGIS Online credits); no local algorithm.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/enrich.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/enrich.html

### Cartography: Collapse Dual Lines To Centerline, Collapse Hydro Polygon, Collapse Road Detail, Merge Divided Roads, Thin Road Network, Aggregate Polygons ...
- Tools covered (10): Aggregate Points; Aggregate Polygons; Collapse Dual Lines To Centerline; Collapse Hydro Polygon; Collapse Road Detail; Create Cartographic Partitions; Delineate Built-Up Areas; Merge Divided Roads; Simplify Building; Thin Road Network
- Difficulty XL; exact reproduction: N
- Why: Cartographic generalisation with undocumented heuristics (pairing, collapse, thinning, orthogonality) and scale/symbol dependence.
- Documented: Documented behaviour only: Collapse Road Detail collapses road configurations whose open-area diameter is <= Collapse Distance into a simple intersection; Merge Divided Roads merges parallel same-class segments within the merge distance into one centerline; Thin Road Network flags (does not delete) segments removable without changing connectivity/density using a Minimum Length; Collapse Dual Lines To Centerline works for near parallel pairs and emits LnType 1/2/3 with LeftLn_FID/RightLn_FID; Aggregate Polygons joins polygons within an aggregation distance, orthogonal or not, with barriers; Delineate Built-Up Areas clusters buildings by a grouping distance. Algorithms are not given.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-collapse-road-detail-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-merge-divided-roads-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-thin-road-network-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/collapse-dual-lines-to-centerline.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/aggregate-polygons.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/delineate-built-up-areas.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/collapse-hydro-polygon.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/simplify-building.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/aggregate-points.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/aggregate-polygons.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/collapse-dual-lines-to-centerline.html ...

### Cartography: Cul-De-Sac Masks, Feature Outline Masks, Intersecting Layers Masks, Convert Labels To Annotation, Tiled Labels To Annotation, Contour Annotation ...
- Tools covered (8): Annotate Selected Features; Contour Annotation; Convert Labels To Annotation; Convert Labels To Graphics; Cul-De-Sac Masks; Feature Outline Masks; Intersecting Layers Masks; Tiled Labels To Annotation
- Difficulty XL; exact reproduction: N
- Why: Result geometry depends on symbol rendering and the Maplex/Standard label engine placement.
- Documented: Labeling engines are described as options (position, fitting strategy, conflict resolution); placement algorithm is not published.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/mapping/text/label-with-the-maplex-label-engine.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-feature-outline-masks-and-intersecting-layers-masks-work.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/annotate-selected-features.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/contour-annotation.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/convert-labels-to-annotation.html ...

### Cartography: Generate Contiguous Cartogram, Disperse Markers, Generate Hachures For Defined Slopes, Create Overpass, Create Underpass, Calculate Polygon Main Angle ...
- Tools covered (8): Calculate Color Theorem Field; Calculate Line Caps; Calculate Polygon Main Angle; Create Overpass; Create Underpass; Disperse Markers; Generate Contiguous Cartogram; Generate Hachures For Defined Slopes
- Difficulty L; exact reproduction: N
- Why: Cartographic refinement algorithms not documented.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/calculate-color-theorem-field.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/calculate-line-caps.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/calculate-polygon-main-angle.html ...

### Cartography: Resolve Road Conflicts, Resolve Building Conflicts, Detect Graphic Conflict, Propagate Displacement
- Tools covered (4): Detect Graphic Conflict; Propagate Displacement; Resolve Building Conflicts; Resolve Road Conflicts
- Difficulty XL; exact reproduction: N
- Why: Symbol-geometry-aware displacement solver; needs symbol rendering extents; algorithm undocumented.
- Documented: Resolve Road Conflicts ranks features by Hierarchy Field and displaces them so symbol edges keep a 0.3 mm visual gap at the reference scale (unlike features: higher-value moves; like features: parallel and overlapping displaced); displacement can be saved as polygons for Propagate Displacement. Resolve Building Conflicts moves, rotates, resizes or hides buildings against barriers per a specification.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-resolve-road-conflicts-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-resolve-building-conflicts-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/detect-graphic-conflict.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/propagate-displacement.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/resolve-building-conflicts.html ...

### Cartography: Simplify Line, Simplify Polygon, Simplify Shared Edges
- Tools covered (3): Simplify Line; Simplify Polygon; Simplify Shared Edges
- Difficulty L; exact reproduction: M
- Why: Two algorithms are named (Douglas-Peucker; Wang-Muller bend simplify) but topological-error handling, shared-edge treatment and resolving tolerances are not specified.
- Documented: POINT_REMOVE: Douglas-Peucker (1973): connect endpoints with a trend line, measure perpendicular distance of vertices, drop those within tolerance, split at the farthest vertex and recurse. BEND_SIMPLIFY: retain critical bends (Wang-Mueller).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-simplify-line-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/simplify-line.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/simplify-polygon.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/simplify-shared-edges.html

### Cartography: Smooth Line, Smooth Polygon, Smooth Shared Edges
- Tools covered (3): Smooth Line; Smooth Polygon; Smooth Shared Edges
- Difficulty L; exact reproduction: M
- Why: PAEK and Bezier interpolation are named with references, but PAEK kernel parameterisation and vertex sampling of the output are not given.
- Documented: PAEK: Polynomial Approximation with Exponential Kernel (Bodansky et al 2002): parametric continuous averaging; each output point is a weighted average of all source points with weights decaying with along-line distance, plus second-degree polynomial approximation; one parameter (Smoothing Tolerance = length of moving path). BEZIER_INTERPOLATION: Bezier segments through vertices.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/how-smooth-line-and-smooth-polygon-work.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/smooth-line.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/smooth-polygon.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/cartography/smooth-shared-edges.html

### Conversion: BIM File To Geodatabase, Extract BIM File Floorplan
- Tools covered (2): BIM File To Geodatabase; Extract BIM File Floorplan
- Difficulty XL; exact reproduction: N
- Why: Revit RVT is proprietary.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/bimfile-to-geodatabase.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/extract-bim-file-floorplan.html

### Conversion: Export To CAD, CAD To Geodatabase, Add CAD Fields
- Tools covered (3): Add CAD Fields; CAD To Geodatabase; Export To CAD
- Difficulty XL; exact reproduction: N
- Why: DWG/DGN are proprietary formats (only DXF is documented); needs a licensed reader/writer.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/projects/supported-data-types-and-items.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/add-cad-fields.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/cad-to-geodatabase.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/export-to-cad.html

### Conversion: Extract Locations From Document, Extract Locations From Text
- Tools covered (2): Extract Locations From Document; Extract Locations From Text
- Difficulty XL; exact reproduction: N
- Why: NLP/AI model based.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/extract-locations-from-document.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/extract-locations-from-text.html

### Conversion: Extract Scanned Lines, Extract Scanned Polygons, Raster to Polyline
- Tools covered (3): Extract Scanned Lines; Extract Scanned Polygons; Raster to Polyline
- Difficulty L; exact reproduction: N
- Why: Raster vectorisation/centerline extraction algorithm and generalisation are undocumented.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/extract-scanned-lines.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/extract-scanned-polygons.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/raster-to-polyline.html

### Conversion: KML To Layer, Layer To KML, Map To KML, KML To Geodatabase
- Tools covered (4): KML To Geodatabase; KML To Layer; Layer To KML; Map To KML
- Difficulty L; exact reproduction: M
- Why: KML styling/symbol rendering to KML is renderer dependent.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/kml-conversion.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/kml-to-geodatabase.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/kml-to-layer.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/layer-to-kml.html ...

### Conversion: Raster to Polygon
- Tools covered (1): Raster to Polygon
- Difficulty L; exact reproduction: M
- Why: SIMPLIFY smoothing is not vertex weeding and is undocumented (ROADMAP: only NO_SIMPLIFY done).
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/raster-to-polygon.html

### Data Management: Add Attribute Rule, Alter Attribute Rule, Delete Attribute Rule, Disable Attribute Rules, Enable Attribute Rules, Evaluate Rules ...
- Tools covered (14): Add Attribute Rule; Alter Attribute Rule; Calculate Geometry Fields Attribute Rule; Delete Attribute Rule; Disable Attribute Rules; Enable Attribute Rules; Evaluate Rules; Export Attribute Rules; Generate ID Attribute Rule; Generate Spatial Join Attribute Rule; Generate Symbol Rotation Attribute Rule; Generate Update Related Attribute Rule; Import Attribute Rules; Reorder Attribute Rule
- Difficulty XL; exact reproduction: M
- Why: Attribute rules are Arcade scripts executed inside the geodatabase on edit; needs an Arcade interpreter plus geodatabase edit hooks and rule storage.
- Documented: Calculation, constraint and validation rule types; scripts use Arcade with rule dictionary keywords.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/an-overview-of-attribute-rules.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/author-attribute-rule-script-expressions.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-attribute-rule.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/alter-attribute-rule.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/calculate-geometry-fields-attribute-rule.html ...

### Data Management: Compress File Geodatabase Data, Uncompress File Geodatabase Data, Compact, Recover File Geodatabase
- Tools covered (4): Compact; Compress File Geodatabase Data; Recover File Geodatabase; Uncompress File Geodatabase Data
- Difficulty L; exact reproduction: M
- Why: Compressed (.cdf) file geodatabase tables use a proprietary compression format with no public spec (ROADMAP: not implemented).
- Documented: Recover File Geodatabase: opens each table in the damaged directory, reads the file header and copies the table to a new FGDB; names kept if system tables survive, otherwise new names; only simple feature classes and tables recovered.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/how-recover-file-geodatabase-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/compact.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/compress-file-geodatabase-data.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/recover-file-geodatabase.html ...

### Data Management, Image Analyst, Reality Mapping: Compute Pansharpen Weights, Create Pansharpened Raster Dataset, Create Ortho Corrected Raster Dataset, Apply Block Adjustment, Compute Block Adjustment, Compute Camera Model ...
- Tools covered (27): Analyze Control Points; Apply Block Adjustment; Build Stereo Model; Compute Block Adjustment; Compute Camera Model; Compute Control Points; Compute Depth Map; Compute Fiducials; Compute Pansharpen Weights; Compute Tie Points; Create Ortho Corrected Raster Dataset; Create Pansharpened Raster Dataset; Detect Control Points; Generate Point Cloud; Interpolate From Point Cloud; Match Control Points; Reconstruct Surface; Update Interior Orientation
- Difficulty XL; exact reproduction: N
- Why: Photogrammetry (bundle adjustment, dense matching, SfM) is heavy numerics with undocumented specifics.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/analyze-control-points.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/apply-block-adjustment.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/build-stereo-model.html ...

### Data Management: Consolidate Map, Package Map, Package Project, Consolidate Project, Consolidate Layer, Package Layer ...
- Tools covered (17): Apply Symbology From Layer; Consolidate Layer; Consolidate Map; Consolidate Project; Create Map Tile Package; Create Mobile Map Package; Create Mobile Scene Package; Create Vector Tile Package; Export Layouts; Export Report To Excel; Export Report To PDF; Match Layer Symbology To A Style; Package Layer; Package Map; Package Project; Save To Layer File; Share Package
- Difficulty XL; exact reproduction: N
- Why: Depends on project/CIM documents (.aprx, .lyrx, .mapx), symbology and the rendering engine.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/apply-symbology-from-layer.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/consolidate-layer.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/consolidate-map.html ...

### Data Management: Create Mobile Geodatabase, Create SQLite Database
- Tools covered (2): Create Mobile Geodatabase; Create SQLite Database
- Difficulty L; exact reproduction: M
- Why: Mobile geodatabase = SQLite with Esri spatial type/ST_Geometry-like functions; schema not published.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-mobile-geodatabase.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-sqlite-database.html

### Data Management: Create Mosaic Dataset, Add Rasters To Mosaic Dataset, Build Footprints, Build Boundary, Build Seamlines, Color Balance Mosaic Dataset ...
- Tools covered (20): Add Rasters To Mosaic Dataset; Alter Mosaic Dataset Schema; Analyze Mosaic Dataset; Build Boundary; Build Footprints; Build Overviews; Build Seamlines; Calculate Cell Size Ranges; Color Balance Mosaic Dataset; Compute Dirty Area; Compute Mosaic Candidates; Create Mosaic Dataset; Create Referenced Mosaic Dataset; Define Overviews; Generate Exclude Area; Generate Raster Collection; Merge Mosaic Dataset Items; Set Mosaic Dataset Properties; Split Mosaic Dataset Items; Synchronize Mosaic Dataset
- Difficulty XL; exact reproduction: N
- Why: Mosaic datasets are proprietary geodatabase structures (raster types, function chains, seamline/color-balance algorithms) with no published algorithm.
- Documented: Documented only functionally: seamlines define edges along which images are mosaicked and remove overlap; the Seamline method is stable while panning versus Closest To Center. Build Seamlines algorithm is not described.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/mosaic-dataset-seamlines-pro-.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/mosaicking-rules-for-a-mosaic-dataset-pro-.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-rasters-to-mosaic-dataset.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/alter-mosaic-dataset-schema.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/analyze-mosaic-dataset.html ...

### Data Management: Create Relationship Class, Table To Relationship Class, Migrate Relationship Class, Add Rule To Relationship Class, Remove Rule From Relationship Class, Set Relationship Class Split Policy ...
- Tools covered (19): Add Attribute Index; Add Global IDs; Add Join; Add Relate; Add Rule To Relationship Class; Add Subtype; Alter Subtype; Create Feature Class; Create Feature Dataset; Create File Geodatabase; Create Relationship Class; Create Table; Migrate Relationship Class; Remove Rule From Relationship Class; Remove Subtype; Set Default Subtype; Set Relationship Class Split Policy; Set Subtype Field; Table To Relationship Class
- Difficulty L; exact reproduction: M
- Why: Writing into existing file geodatabases (system tables, GDB_Items XML, indexes) is the standing structural blocker (ROADMAP: add to existing GDB not done).
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-attribute-index.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-global-ids.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-join.html ...

### Data Management: Create Topology, Add Feature Class To Topology, Add Rule To Topology, Validate Topology, Export Topology Errors, Set Cluster Tolerance ...
- Tools covered (8): Add Feature Class To Topology; Add Rule To Topology; Create Topology; Export Topology Errors; Remove Feature Class From Topology; Remove Rule From Topology; Set Cluster Tolerance; Validate Topology
- Difficulty XL; exact reproduction: M
- Why: Geodatabase topology storage (dirty areas, error features, exceptions in system tables) and validate semantics are not published; rules are listed per geometry type with predefined fixes.
- Documented: Rules are documented per geometry type, e.g. polygon: Must Be Larger Than Cluster Tolerance (mandatory), Must Not Overlap, ... Each rule has errors and predefined fixes. Topology status (errors, exceptions) is saved in the geodatabase. Validate algorithm itself is not described.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/editing/geodatabase-topology-rules-for-polygon-features.html ; https://doc.esri.com/en/arcgis-pro/latest/help/editing/geodatabase-topology-rules-for-point-features.html ; https://doc.esri.com/en/arcgis-pro/latest/help/editing/geodatabase-topology-rules-for-polyline-features.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-feature-class-to-topology.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-rule-to-topology.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-topology.html ...

### Data Management: Create Version, Reconcile Versions, Register As Versioned, Unregister As Versioned, Alter Version, Change Version ...
- Tools covered (39): Add Field Conflict Filter; Alter Version; Analyze Datasets; Change Version; Compare Replica Schema; Compress; Create Enterprise Geodatabase; Create Replica; Create Replica From Server; Create Version; Delete Version; Diagnose Version Metadata; Diagnose Version Tables; Disable Archiving; Disable Replica Tracking; Enable Archiving; Enable Enterprise Geodatabase; Enable Replica Tracking; Export Acknowledgement Message; Export Data Change Message; Export Replica Schema; Import Message; Import Replica Schema; Migrate Object ID To 64 Bit; Migrate Storage; Prune Branch History; Re-export Unacknowledged Messages; Rebuild Indexes; Reconcile Versions; Register As Versioned; Remove Field Conflict Filter; Repair Version Metadata; Repair Version Tables; Synchronize Changes; Trim Archive History; Unregister As Versioned; Unregister Replica; Upgrade Dataset; Upgrade Geodatabase
- Difficulty XL; exact reproduction: N
- Why: Enterprise (SQL/Oracle/PostgreSQL) geodatabase machinery: versioning, replication and archiving rely on RDBMS-side schema and stored procedures that Esri does not publish.
- Documented: Documented concepts only: traditional vs branch versioning, version trees, reconcile/post, replica messages.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/overview-of-versioning-in-arcgis-pro.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-field-conflict-filter.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/alter-version.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/analyze-datasets.html ...

### Analysis, Data Management: Integrate, Pairwise Integrate
- Tools covered (2): Integrate; Pairwise Integrate
- Difficulty L; exact reproduction: M
- Why: Vertex snapping/cracking within XY tolerance is not specified beyond the cluster-tolerance definition (cluster processing order undocumented).
- Documented: Cracking inserts vertices at intersections of feature edges; clustering snaps vertices within XY tolerance (as described for Intersect/Union).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/how-intersect-analysis-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/pairwise-integrate.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/integrate.html

### Data Management: Make Query Layer, Make Query Table, Make Aggregation Query Layer, Create Database Connection, Create Spatial Type
- Tools covered (5): Create Database Connection; Create Spatial Type; Make Aggregation Query Layer; Make Query Layer; Make Query Table
- Difficulty XL; exact reproduction: N
- Why: Requires RDBMS drivers/connections (SQL Server, Oracle, PostgreSQL, SAP HANA ...).
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-database-connection.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-spatial-type.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/make-aggregation-query-layer.html ...

### Data Management: Mosaic, Mosaic To New Raster, Copy Raster, Clip Raster, Composite Bands, Split Raster ...
- Tools covered (16): Batch Build Pyramids; Batch Calculate Statistics; Build Pyramids; Build Pyramids And Statistics; Build Raster Attribute Table; Calculate Statistics; Clip Raster; Composite Bands; Copy Raster; Create Raster Dataset; Extract Subdataset; Mosaic; Mosaic To New Raster; Raster To DTED; Split Raster; Workspace To Raster Dataset
- Difficulty L; exact reproduction: M
- Why: Mosaic operators/colormap modes, pyramid resampling and statistics skip factors, and raster format conversions (CRF, MRF, Esri Grid, IMG, JPEG2000) multiply the surface; blending details undocumented.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/supported-raster-dataset-file-formats.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/batch-build-pyramids.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/batch-calculate-statistics.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/build-pyramids-and-statistics.html ...

### Data Management: Project Raster, Warp, Warp From File, Rotate, Shift, Rescale ...
- Tools covered (9): Flip; Mirror; Project Raster; Register Raster; Rescale; Rotate; Shift; Warp; Warp From File
- Difficulty L; exact reproduction: M
- Why: Output grid/cell-size derivation, transformation (polynomial, spline, adjust, projective) fitting and resampling kernels are only described in outline.
- Documented: Warp: determines output extent, sets rows/cols about equal to the input, default cell size = extent / (rows, cols), which the resampling algorithm then uses. Project Raster page defines projection parameters (semi-axes, standard parallels, ...) but not the resampling pipeline.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/how-warp-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/how-project-raster-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/flip.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/mirror.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/project-raster.html ...

### Data Management, Editing: Align Features, Edgematch Features, Generate Edgematch Links, Generate Rubbersheet Links, Rubbersheet Features, Calculate Transformation Errors ...
- Tools covered (11): Align Features; Calculate Transformation Errors; Detect Feature Changes; Edgematch Features; Feature Compare; Generate Edgematch Links; Generate Rubbersheet Links; Rubbersheet Features; Split Line By Match; Transfer Attributes; Transform Features
- Difficulty XL; exact reproduction: N
- Why: Feature matching and rubbersheet adjustment are heuristic (match table scoring, link generation, local transformation); algorithms not published.
- Documented: Feature matching finds corresponding features in two similar datasets based on a search distance and records results in a match table; used by Generate Rubbersheet Links, Transfer Attributes and Detect Feature Changes.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/about-feature-matching-and-the-match-table.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/align-features.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/rubbersheet-features.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/detect-feature-changes.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/feature-compare.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/align-features.html ...

### Geocoding: Batch Geocode, Geocode File, Geocode Locations From Table
- Tools covered (3): Batch Geocode; Geocode File; Geocode Locations From Table
- Difficulty XL; exact reproduction: N
- Why: Calls the ArcGIS World Geocoding Service (online, credits).
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/geocode-addresses.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/batch-geocode.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/geocode-file.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/geocode-locations-from-table.html

### Geocoding: Create Locator, Create Feature Locator, Create Composite Address Locator, Rebuild Address Locator, Clip Locator, Geocode Addresses ...
- Tools covered (16): Add Polygon Fields To Locator; Assign Streets To Points; Assign Zones To Streets; Clip Locator; Create Address Locator; Create Composite Address Locator; Create Feature Locator; Create Locator; Delete Polygon Fields From Locator; Geocode Addresses; Rebuild Address Locator; Rematch Addresses; Reverse Geocode; Split Address Into Components; Split Match Narrative Into Fields; Standardize Addresses
- Difficulty XL; exact reproduction: N
- Why: Locator (.loc/.loz) is a proprietary index; address parsing, standardisation, candidate scoring and country-specific rules are undocumented; World Geocoding Service is online.
- Documented: Locators hold a snapshot of reference data plus indexes and local addressing knowledge returning the best match; roles/fields configure what is indexed.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geocoding/about-locators.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/geocoding/fundamentals-of-creating-a-locator.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/geocoding/introduction-to-locator-roles.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/add-polygon-fields-to-locator.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/assign-streets-to-points.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geocoding/assign-zones-to-streets.html ...

### Geostatistical Analyst: Create Geostatistical Layer, GA Layer To Grid, GA Layer To Points, GA Layer To Contour, GA Layer To Rasters, GA Layer 3D To Multidimensional Raster ...
- Tools covered (10): Calculate Z-value; Create Geostatistical Layer; GA Layer 3D To Multidimensional Raster; GA Layer 3D To NetCDF; GA Layer To Contour; GA Layer To Grid; GA Layer To Points; GA Layer To Rasters; Get Model Parameter; Set Model Parameter
- Difficulty L; exact reproduction: N
- Why: Geostatistical layer (.lyrx/.ga) is a proprietary model container.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/analysis/geostatistical-analyst/what-is-a-geostatistical-layer-.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/calculate-z-value.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/create-geostatistical-layer.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/ga-layer-3d-to-multidimensional-raster.html ...

### Geostatistical Analyst: Diffusion Interpolation With Barriers, Kernel Interpolation With Barriers, Radial Basis Functions, Local Polynomial Interpolation, Global Polynomial Interpolation, IDW 3D ...
- Tools covered (8): Areal Interpolation Layer To Polygons; Diffusion Interpolation With Barriers; Global Polynomial Interpolation; IDW 3D; Kernel Interpolation With Barriers; Local Polynomial Interpolation; Nearest Neighbor 3D; Radial Basis Functions
- Difficulty L; exact reproduction: M
- Why: Barrier-aware interpolators (diffusion kernel via finite-element mesh) have limited public documentation.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/areal-interpolation-layer-to-polygons.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/diffusion-interpolation-with-barriers.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/global-polynomial-interpolation.html ...

### Geostatistical Analyst: Empirical Bayesian Kriging, Empirical Bayesian Kriging 3D, EBK Regression Prediction, Moving Window Kriging, Gaussian Geostatistical Simulations, Semivariogram Sensitivity ...
- Tools covered (10): Compare Geostatistical Layers; Cross Validation; EBK Regression Prediction; Empirical Bayesian Kriging; Empirical Bayesian Kriging 3D; Exploratory Interpolation; Gaussian Geostatistical Simulations; Moving Window Kriging; Neighborhood Selection; Semivariogram Sensitivity
- Difficulty XL; exact reproduction: N
- Why: EBK subsets the data and runs many simulations of semivariograms (random); results are stochastic and the exact subsetting/simulation scheme is not given.
- Documented: EBK automates semivariogram estimation through subsetting and simulations and accounts for the error of semivariogram estimation (other kriging uses one fitted semivariogram), giving more accurate standard errors; only Standard/Smooth Circular neighbourhoods; execution time grows with subset size and overlap factor.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/analysis/geostatistical-analyst/what-is-empirical-bayesian-kriging-.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/empirical-bayesian-kriging.html ; https://doc.esri.com/en/arcgis-pro/latest/help/analysis/geostatistical-analyst/kriging-in-geostatistical-analyst.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/compare-geostatistical-layers.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/cross-validation.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/geostatistical-analyst/ebk-regression-prediction.html ...

### Image Analyst, Spatial Analyst: Analyze Changes Using CCDC, Analyze Changes Using LandTrendr, Detect Change Using Change Analysis Raster, Compute Change Raster, Generate Trend Raster, Predict Using Trend Raster ...
- Tools covered (18): Aggregate Multidimensional Raster; Analyze Changes Using CCDC; Analyze Changes Using LandTrendr; Compute Change Raster; Detect Change Using Change Analysis Raster; Dimensional Moving Statistics; Find Argument Statistics; Generate Multidimensional Anomaly; Generate Trend Raster; Interpolate From Spatiotemporal Points; Multidimensional Principal Components; Multidimensional Raster Correlation; Optimal Interpolation; Predict Using Trend Raster; Summarize Categorical Raster
- Difficulty XL; exact reproduction: M
- Why: Published research algorithms (CCDC, LandTrendr) with Esri-specific parameters and CRF/multidimensional outputs.
- Documented: CCDC: Continuous Change Detection and Classification (Zhu and Woodcock 2014) models seasonal, trend and break components per pixel over an image stack. LandTrendr (Kennedy et al 2010): segments pixel trajectories into disturbance and recovery. Optimal Interpolation: data assimilation merging background data with observations.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/how-analyze-changes-using-ccdc-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/how-analyze-changes-using-landtrendr-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/learn-more-about-optimal-interpolation.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/aggregate-multidimensional-raster.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/analyze-changes-using-ccdc.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/analyze-changes-using-landtrendr.html ...

### Image Analyst: Apply Complex Data Filter, Apply Coregistration, Apply Geometric Terrain Correction, Apply Orbit Correction, Apply Radiometric Calibration, Apply Radiometric Terrain Flattening ...
- Tools covered (23): Apply Complex Data Filter; Apply Coregistration; Apply Geometric Terrain Correction; Apply Orbit Correction; Apply Radiometric Calibration; Apply Radiometric Terrain Flattening; Compute Coherence; Compute SAR Indices; Convert SAR Units; Deburst; Despeckle; Detect Bright Ocean Objects; Detect Dark Ocean Areas; Download Orbit File; Extract Ocean Winds; Extract Water; Generate Interferogram; Generate Masked SAR Raster; Generate Multitemporal Coherence; Generate Radiometric Terrain Corrected Data; Multilook; Remove Thermal Noise; Unwrap Phase
- Difficulty XL; exact reproduction: N
- Why: SAR processing chain (calibration, terrain correction, InSAR, phase unwrapping) is heavy numerics with undocumented implementation choices.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/apply-complex-data-filter.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/apply-coregistration.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/apply-geometric-terrain-correction.html ...

### 3D Analyst, GeoAI, Image Analyst, Raster Analysis, Reality Mapping, Spatial Analyst: Classify Objects Using Deep Learning, Classify Pixels Using Deep Learning, Detect Objects Using Deep Learning, Detect Change Using Deep Learning, Translate Pixels Using Deep Learning, Train Deep Learning Model ...
- Tools covered (46): Classify Objects Using Deep Learning; Classify Pixels Using Deep Learning; Classify Point Cloud Using Trained Model; Classify Text Using Deep Learning; Compute Accuracy For Object Detection; Compute Accuracy For Pixel Classification; Deep Learning Model To Ecd; Detect Change Using Deep Learning; Detect Control Points; Detect Objects From Point Cloud Using Trained Model; Detect Objects Using Deep Learning; Evaluate Point Cloud Classification Model; Export Training Data For Deep Learning; Extract Embedding To Fields; Extract Entities Using Deep Learning; Extract Features Using AI Models; Find Similar Features Using Embeddings; Forecast Using Time Series Model; Generate Embeddings Using AI Models; Merge Embeddings; Non Maximum Suppression; Predict Missing Values Using AI Model; Predict Using AutoML; Prepare Point Cloud Object Detection Training Data; Prepare Point Cloud Training Data; Process Text Using AI Model; Train Deep Learning Model; Train Entity Recognition Model; Train Point Cloud Classification Model; Train Point Cloud Object Detection Model; Train Text Classification Model; Train Text Transformation Model; Train Time Series Forecasting Model; Train Using AutoDL; Train Using AutoML; Transform Image Shapes; Transform Text Using Deep Learning; Translate Pixels Using Deep Learning
- Difficulty XL; exact reproduction: N
- Why: Deep learning / AI model training and inference (.dlpk/.emd models, PyTorch/TensorFlow runtimes, GPUs); outputs are model dependent.
- Documented: AutoDL: automates data preparation, model selection, hyperparameter tuning and evaluation. Pixel classification, object detection and object classification pages describe model architectures and arguments.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/how-autodl-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/overview-of-the-deep-learning-models.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/deep-learning-arguments.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/classify-point-cloud-using-trained-model.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/detect-objects-from-point-cloud-using-trained-model.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/evaluate-point-cloud-classification-model.html ...

### Image Analyst: Convert Video Metadata, Extract Video Frames To Images, Generate Video Metadata (Stationary), Video Multiplexer, Video Metadata To Feature Class
- Tools covered (5): Convert Video Metadata; Extract Video Frames To Images; Generate Video Metadata (Stationary); Video Metadata To Feature Class; Video Multiplexer
- Difficulty XL; exact reproduction: N
- Why: Video codecs and KLV/FMV metadata handling.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/convert-video-metadata.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/extract-video-frames-to-images.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/generate-video-metadata.html ...

### Image Analyst, Spatial Analyst: Raster Calculator
- Tools covered (2): Raster Calculator
- Difficulty L; exact reproduction: M
- Why: Map-algebra expression semantics (operator typing, NoData propagation, integer/float promotion, Python-like grammar) are spread across many pages.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/how-raster-calculator-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/building-expressions-in-raster-calculator.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/raster-calculator.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/raster-calculator.html

### Image Analyst, Spatial Analyst: Segment Mean Shift, Compute Segment Attributes, Remove Raster Segment Tiling Artifacts, Train Random Trees Classifier, Train Support Vector Machine Classifier, Train K-Nearest Neighbor Classifier ...
- Tools covered (26): Classify Raster; Classify Raster Using Spectra; Compute Segment Attributes; Detect Image Anomalies; Detect Target Using Spectra; Linear Spectral Unmixing; Predict Using Regression Model; Reduce Spectral Bands; Remove Raster Segment Tiling Artifacts; Segment Mean Shift; Train ISO Cluster Classifier; Train K-Nearest Neighbor Classifier; Train Maximum Likelihood Classifier; Train Random Trees Classifier; Train Random Trees Regression Model; Train Support Vector Machine Classifier
- Difficulty XL; exact reproduction: N
- Why: Segmentation specifics and trained classifiers (random trees, SVM, KNN) are stochastic or undocumented; .ecd/.gsg definition files are Esri formats.
- Documented: Understanding segmentation and classification page describes workflows only; Segment Mean Shift tool page lists parameters (spectral/spatial detail, minimum segment size) without the algorithm.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/understanding-segmentation-and-classification.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/segment-mean-shift.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/classify-raster-using-spectra.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/classify-raster.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/compute-segment-attributes.html ...

### Network Analyst: Create Network Dataset, Build Network, Create Network Dataset From Template, Create Template From Network Dataset, Dissolve Network, Export Network Travel Modes ...
- Tools covered (14): Build Network; Create Network Dataset; Create Network Dataset From Template; Create Template From Network Dataset; Create Turn Feature Class; Dissolve Network; Export Network Travel Modes; Increase Maximum Edges; Make Network Dataset Layer; Populate Alternate ID Fields; Replace Network Travel Modes; Turn Table To Turn Feature Class; Update by Alternate ID Fields; Update by Geometry
- Difficulty XL; exact reproduction: M
- Why: Network dataset (.nd) elements, connectivity policies, attributes, evaluators and travel modes are a proprietary geodatabase structure; build semantics are only described functionally.
- Documented: Dissolve Network: merges logically connected, identical line features (same street name, hierarchy, restrictions ...) into fewer lines in a new network dataset.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/how-dissolve-network-works.html ; https://doc.esri.com/en/arcgis-pro/latest/help/analysis/networks/what-is-network-dataset-.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/build-network.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/create-network-dataset-from-template.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/create-network-dataset.html ...

### Network Analyst: Solve, Make Route Analysis Layer, Make Closest Facility Analysis Layer, Make Service Area Analysis Layer, Make OD Cost Matrix Analysis Layer, Make Location-Allocation Analysis Layer ...
- Tools covered (12): Add Locations; Calculate Locations; Directions; Make Closest Facility Analysis Layer; Make Last Mile Delivery Analysis Layer; Make Location-Allocation Analysis Layer; Make OD Cost Matrix Analysis Layer; Make Route Analysis Layer; Make Service Area Analysis Layer; Make Vehicle Routing Problem Analysis Layer; Make Waste Collection Analysis Layer; Solve
- Difficulty XL; exact reproduction: M
- Why: Route/Closest Facility/OD/Service Area use Dijkstra (documented) but TSP/VRP (tabu search) and location-allocation metaheuristics are explicitly proprietary; turn-by-turn directions generation and hierarchical routing heuristics are not reproducible from docs.
- Documented: Route, Closest Facility, OD Cost Matrix and Service Area solvers are based on Dijkstra with one-way, turn, junction-impedance, barrier and side-of-street handling; two path types (exact shortest path and a hierarchical bidirectional Dijkstra heuristic that does not guarantee the shortest path). TSP: OD matrix + tabu search (proprietary implementation) with time windows. VRP: OD matrix, insertion-built initial solution, improved by resequencing/relocating/exchanging orders under a tabu search metaheuristic (proprietary). Service area polygons: buffering of traversed lines (or detailed polygons closer to traversed than non-traversed lines). Location-Allocation: OD matrix, Hillsman editing, semi-random solutions refined by Teitz-Bart vertex substitution, combined by a metaheuristic.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/analysis/networks/algorithms-used-by-network-analyst.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/add-locations.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/calculate-locations.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/directions.html ...

### Raster Analysis, Spatial Analyst: Distance Accumulation, Distance Allocation, Optimal Path As Line, Optimal Path As Raster, Least Cost Corridor, Optimal Region Connections ...
- Tools covered (31): Calculate Distance; Calculate Travel Cost; Corridor; Cost Allocation; Cost Back Link; Cost Connectivity; Cost Distance; Cost Path; Cost Path As Polyline; Determine Optimum Travel Cost Network; Determine Travel Cost Path As Polyline; Determine Travel Cost Paths To Destinations; Distance Accumulation; Distance Allocation; Euclidean Allocation; Euclidean Back Direction; Euclidean Direction; Euclidean Distance; Least Cost Corridor; Optimal Path As Line; Optimal Path As Raster; Optimal Region Connections; Path Distance; Path Distance Allocation; Path Distance Back Link
- Difficulty XL; exact reproduction: M
- Why: New tools reconstruct a continuous accumulative surface from slope (not a graph search); the update scheme (wavefront order, vertical/horizontal factor integration, barrier handling) is only outlined. Legacy tools use a Dijkstra-style network over cell centres with documented edge-weight formulas.
- Documented: Distance Accumulation/Allocation: algorithm reconstructs a surface from its slope instead of using a cell-centre network, giving distortion-free results; least-cost paths are paths of steepest descent. Legacy cost distance: cost to move between cells = cost of travel x characteristics of mover and movement; edge weights from cell pairs on a network connected through cell centres; shortest-path algorithm on that network. Path distance adds surface distance and horizontal/vertical factors. (Cost Distance family is deprecated in favour of Distance Accumulation.)
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/distance-accumulation-algorithm.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-the-cost-distance-tools-work.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-the-path-distance-tools-work.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/connect-regions-with-the-optimal-network.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/calculate-distance.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/calculate-travel-cost.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/cost-path-as-polyline.html ...

### Raster Analysis, Spatial Analyst: Flow Direction, Flow Accumulation, Fill, Sink, Flow Distance, Flow Length ...
- Tools covered (28): Adjust Raster to Stream; Adjust Stream to Raster; Basin; Derive Continuous Flow; Derive Stream As Line; Derive Stream As Raster; Fill; Flow Accumulation; Flow Direction; Flow Distance; Flow Length; Generate Breach Lines; Geodesic Flow Direction; Locate Depressions; Sink; Snap Pour Point; Storage Capacity; Stream Link; Stream Order; Stream to Feature; Validate Flow Direction; Watershed
- Difficulty L; exact reproduction: M
- Why: D8 core is documented and implemented; flat-area resolution, Derive Continuous Flow (iterative sink handling), Stream to Feature vectorisation and breach-line detection are not specified.
- Documented: Flow Direction: D8 (Jenson and Domingue 1988): steepest descent maximum_drop = dz/distance between cell centres, eight direction codes, optional drop raster and edge outflow. Watershed: contributing area above pour points (features or flow accumulation threshold at stream junctions).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-flow-direction-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-watershed-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-derive-continuous-flow-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-fill-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/fill.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/flow-accumulation.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/raster-analysis/flow-direction.html ...

### Space Time Pattern Mining: Create Space Time Cube By Aggregating Points, Create Space Time Cube From Defined Locations, Create Space Time Cube From Multidimensional Raster Layer, Emerging Hot Spot Analysis, Local Outlier Analysis, Time Series Clustering ...
- Tools covered (13): Change Point Detection; Create Space Time Cube By Aggregating Points; Create Space Time Cube From Defined Locations; Create Space Time Cube From Multidimensional Raster Layer; Curve Fit Forecast; Emerging Hot Spot Analysis; Evaluate Forecasts By Location; Exponential Smoothing Forecast; Fill Missing Values; Forest-based Forecast; Local Outlier Analysis; Time Series Clustering; Time Series Cross Correlation
- Difficulty XL; exact reproduction: M
- Why: Space-time cube is a netCDF with Esri conventions; Gi*+Mann-Kendall is formula-defined, but forecasting model selection/optimisation and forest-based forecasts are not exactly specified.
- Documented: Emerging Hot Spot Analysis: computes Gi* (with FDR) per cube bin, then evaluates the time series of hot/cold spot z-scores per location with the Mann-Kendall trend test and classifies locations (new, intensifying, diminishing, sporadic ...). Exponential Smoothing Forecast: Holt-Winters decomposition into seasonal and trend components at each location. Forest-based Forecast: forest regression for future time slices.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/learnmoreemerging.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/learnmoreexponentialsmoothingforecast.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/learnmoreforestbasedforecast.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/learnmorecreatecube.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/change-point-detection.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/create-space-time-cube.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/space-time-pattern-mining/createcubefromdefinedlocations.html ...

### Spatial Analyst: Area Solar Radiation, Points Solar Radiation, Feature Solar Radiation, Raster Solar Radiation, Solar Radiation Graphics
- Tools covered (5): Area Solar Radiation; Feature Solar Radiation; Points Solar Radiation; Raster Solar Radiation; Solar Radiation Graphics
- Difficulty XL; exact reproduction: M
- Why: Hemispherical viewshed model with sun-map/sky-map discretisation; exact sector counts, atmospheric transmittivity and diffuse models are documented as equations but ArcGIS-identical results need exact sector geometry and viewshed.
- Documented: Based on the hemispherical viewshed algorithm (Rich 1990; Rich et al 1994; Fu and Rich 2000, 2002). Global = Direct + Diffuse summed over sun-map and sky-map sectors; Dir(theta,alpha) = S_const * beta^m(theta) * SunDur * SunGap * cos(AngIn), etc.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-solar-radiation-is-calculated.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-raster-solar-radiation-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/area-solar-radiation.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/feature-solar-radiation.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/points-solar-radiation.html ...

### Spatial Analyst: Darcy Flow, Darcy Velocity, Particle Track, Porous Puff
- Tools covered (4): Darcy Flow; Darcy Velocity; Particle Track; Porous Puff
- Difficulty L; exact reproduction: M
- Why: Numerical groundwater model; equations documented, discretisation details to be verified.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-darcy-flow-and-darcy-velocity-work.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-particle-track-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-porous-puff-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/darcy-flow.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/darcy-velocity.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/particle-track.html ...

### 3D Analyst, Spatial Analyst: Geomorphon Landforms, Multiscale Surface Deviation, Multiscale Surface Difference, Multiscale Surface Percentile, Topographic Position Index, Surface Parameters ...
- Tools covered (16): Contour List; Contour with Barriers; Cut Fill; Feature Preserving Smoothing; Geomorphon Landforms; Multiscale Surface Deviation; Multiscale Surface Difference; Multiscale Surface Percentile; Surface Area Ratio; Surface Parameters; Top Hat Transform; Topographic Position Index
- Difficulty L; exact reproduction: M
- Why: Based on published methods (Jasiewicz and Stepinski 2013 geomorphons; feature-preserving DEM smoothing) with Esri-specific parameter handling.
- Documented: Geomorphon: classifies terrain by elevation differences and line-of-sight pattern into 498 unique ternary patterns mapped to landform classes. Multiscale Surface Deviation: maximum surface deviation across a range of neighbourhood sizes plus the scale where it occurs.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-geomorphon-landforms-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-multiscale-surface-deviation-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-feature-preserving-smoothing-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/contour-list.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/contour-with-barriers.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/3d-analyst/cut-fill.html ...

### Spatial Analyst: Iso Cluster, Iso Cluster Unsupervised Classification, Maximum Likelihood Classification, Class Probability, Create Signatures, Edit Signatures ...
- Tools covered (9): Band Collection Statistics; Class Probability; Create Signatures; Dendrogram; Edit Signatures; Iso Cluster; Iso Cluster Unsupervised Classification; Maximum Likelihood Classification; Principal Components
- Difficulty L; exact reproduction: M
- Why: Closed-form pieces (MLC, PCA, covariance) are replicable; Iso Cluster iteration/merge/split rules and random initial means are not specified.
- Documented: MLC: assumes normally distributed class samples and Bayes decision; uses class mean vectors and covariance matrices, optional a priori probabilities (equal or file). Iso Cluster: iterative migrating-means (ISODATA-like): assign cells to nearest mean (minimum Euclidean distance), recompute means, repeat.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-maximum-likelihood-classification-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-iso-cluster-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/band-collection-statistics.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/class-probability.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/create-signatures.html ...

### Spatial Statistics: Forest-based and Boosted Classification and Regression, Presence-only Prediction (MaxEnt), Predict Using Spatial Statistics Model File, Evaluate Predictions with Cross-validation, Evaluate Variable Influence for Predictions, Prepare Data for Prediction ...
- Tools covered (13): Build Balanced Zones; Causal Inference Analysis; Density-based Clustering; Estimate Time to Event; Evaluate Predictions with Cross-validation; Evaluate Variable Influence for Predictions; Forest-based and Boosted Classification and Regression; Multivariate Clustering; Predict Using Spatial Statistics Model File; Prepare Data for Prediction; Presence-only Prediction (MaxEnt); Similarity Search; Spatially Constrained Multivariate Clustering
- Difficulty XL; exact reproduction: N
- Why: Machine learning with randomness (random forest per Breiman and Cutler, XGBoost, MaxEnt) or heuristic optimisation (zone balancing, K-means initialisation); outputs not bit-reproducible without Esri internals.
- Documented: Forest-based: adaptation of random forest (Breiman and Cutler) building many trees on random subsets of data and variables, aggregated; or XGBoost boosting. Spatially Constrained Multivariate Clustering: connectivity graph (minimum spanning tree) with the SKATER method plus evidence accumulation. Density-based Clustering: finds concentrations of points separated by sparse areas, noise labelled; optional time dimension.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-forest-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-presence-only-prediction-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-spatially-constrained-multivariate-clustering-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-density-based-clustering-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-multivariate-clustering-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/buildbalancedzones.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/causal-inference-analysis.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/densitybasedclustering.html ...

### Spatial Statistics: Ordinary Least Squares (OLS), Exploratory Regression, Generalized Linear Regression, Spatial Autoregression, Geographically Weighted Regression (GWR), Geographically Weighted Regression ...
- Tools covered (11): Compare Neighborhood Conceptualizations; Create Spatial Component Explanatory Variables; Decompose Spatial Structure (Moran Eigenvectors); Exploratory Regression; Filter Spatial Autocorrelation From Field; Generalized Linear Regression; Geographically Weighted Regression; Geographically Weighted Regression (GWR); Multiscale Geographically Weighted Regression (MGWR); Ordinary Least Squares (OLS); Spatial Autoregression
- Difficulty XL; exact reproduction: M
- Why: OLS is closed form; GWR/MGWR need bandwidth search (AICc golden section) and backfitting; exact ArcGIS bandwidth/solver tolerances unspecified.
- Documented: GWR fits a local regression at every feature using neighbours chosen by Neighborhood Type and Neighborhood Selection Method; supports Gaussian, binomial and Poisson dependent variables; use with several hundred features. MGWR lets each explanatory variable have its own bandwidth. Exploratory Regression tries all combinations of candidate explanatory variables and reports models that pass all OLS diagnostics. Spatial Autoregression fits spatial lag, spatial error or combined models, optionally chosen by diagnostics.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-geographicallyweightedregression-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-multiscale-geographically-weighted-regression-mgwr-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-exploratory-regression-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-ols-regression-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-spatial-autoregression-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/compare-neighborhood-conceptualizations.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/create-spatial-component-explanatory-variables.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/decompose-spatial-structure.html ...

## Part 2. Documented algorithm, moderate work (DOC)

### Analysis: Create Thiessen Polygons
- Tools covered (1): Create Thiessen Polygons
- Difficulty M; exact reproduction: Y
- Why: Algorithm is documented (Delaunay) but output polygon order/vertex order and the 10 percent outer extent need checking against ArcGIS.
- Documented: All points are triangulated into a Delaunay TIN; perpendicular bisectors of triangle edges form the polygon edges; their intersections are the vertices. Outer boundary is the extent of the points plus 10 percent (or the Extent environment). Warned to be unreliable in geographic coordinates. The tool was rewritten at Pro 2.0 for speed (toolbox history).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/create-thiessen-polygons.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/analysis-toolbox-history-pro.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/create-thiessen-polygons.html

### Analysis: Summarize Nearby, Summarize Within, Tabulate Intersection, Apportion Polygon, Count Overlapping Features, Remove Overlap (multiple) ...
- Tools covered (8): Apportion Polygon; Count Overlapping Features; Pairwise Polygon Neighbors; Polygon Neighbors; Remove Overlap (multiple); Summarize Nearby; Summarize Within; Tabulate Intersection
- Difficulty M; exact reproduction: Y
- Why: Overlay/statistics semantics are documented in parameter text only; edge cases (network distance, apportioning by points/length) need ArcGIS comparison.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/how-polygonneighbors-analysis-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/apportion-polygon.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/count-overlapping-features.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/pairwise-polygon-neighbors.html ...

### Conversion: Excel To Table, Table To Excel
- Tools covered (2): Excel To Table; Table To Excel
- Difficulty M; exact reproduction: Y
- Why: xlsx is documented OOXML; legacy xls is binary; type inference rules need ArcGIS comparison.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/excel-to-table.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/conversion/table-to-excel.html

### Data Management, Geostatistical Analyst, Spatial Analyst: Create Random Points, Create Spatially Balanced Points, Create Spatial Sampling Locations, Create Random Raster, Subset Features
- Tools covered (8): Create Random Points; Create Random Raster; Create Spatial Sampling Locations; Create Spatially Balanced Points; Subset Features
- Difficulty M; exact reproduction: M
- Why: Random output is reproducible only if the same generator is used; ArcGIS offers C rand(), ACM algorithm 599 and Mersenne Twister mt19937 with seed; spatially balanced sampling (GRTS-like) algorithm is not documented.
- Documented: Create Random Points: a random stream is created from generator+seed; x and y are drawn from the stream and transformed to uniform over the extent. For polygons the polygon is partitioned into triangles, a triangle is chosen with probability proportional to area, and two legs of the triangle become axes for the draw. Generators: Standard C rand(), ACM collected algorithm 599, Mersenne Twister mt19937.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/how-create-random-points-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/random-number-generators-and-streams.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-random-points.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-random-raster.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/create-spatial-sampling-locations.html ...

### Data Management: Eliminate
- Tools covered (1): Eliminate
- Difficulty M; exact reproduction: M
- Why: Merge target selection documented (longest shared border or largest area) but tie-breaking, repeated passes and attribute handling of the winner polygon are not.
- Documented: Selected polygons are merged into the neighbouring polygon with the longest shared border (default) or the largest area. Selected polygons that touch only other selected polygons cannot be merged and are left (rerun needed). Exclusion Expression/Exclusion Layer protect polygons or boundary portions.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/eliminate.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/eliminate.html

### Data Management: Feature To Polygon, Feature To Line, Polygon To Line, Dice, Subdivide Polygon, Split Line at Point ...
- Tools covered (9): Dice; Feature To Line; Feature To Polygon; Generate Tessellation; Polygon To Line; Split Line At Vertices; Split Line at Point; Subdivide Polygon; Unsplit Line
- Difficulty M; exact reproduction: M
- Why: Geometry results are standard but row order/attribute carry-over (e.g. banded order noted in ROADMAP) is engine specific.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/dice.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/feature-to-line.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/feature-to-polygon.html ...

### Editing: Generalize, Simplify By Straight Lines And Circular Arcs, Simplify By Tangent Segments
- Tools covered (3): Generalize; Simplify By Straight Lines And Circular Arcs; Simplify By Tangent Segments
- Difficulty M; exact reproduction: M
- Why: Generalize = Douglas-Peucker (documented, same as Simplify Line POINT_REMOVE); the two arc/tangent simplifiers are undocumented curve-fitting.
- Documented: Generalize uses the same algorithm as the Simplify Line tool point remove method (Douglas-Peucker), modifies the input record by record.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/generalize.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/generalize.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/simplify-by-tangent-segments.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/editing/simplifybystraightlinesandcirculararcs.html

### Linear Referencing: Calibrate Routes, Create Routes, Locate Features Along Routes, Make Route Event Layer, Overlay Route Events, Dissolve Route Events ...
- Tools covered (7): Calibrate Routes; Create Routes; Dissolve Route Events; Locate Features Along Routes; Make Route Event Layer; Overlay Route Events; Transform Route Events
- Difficulty M; exact reproduction: Y
- Why: Documented functionally; needs M-value geometry and event-table semantics (output field naming, ordering).
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/linear-referencing/calibrate-routes.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/linear-referencing/introduction-to-linear-referencing.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/linear-referencing/calibrate-routes.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/linear-referencing/create-routes.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/linear-referencing/dissolve-route-events.html ...

### Spatial Analyst: Aggregate, Nibble, Region Group, Boundary Clean, Majority Filter, Expand ...
- Tools covered (9): Aggregate; Boundary Clean; Combine; Expand; Majority Filter; Nibble; Region Group; Shrink; Thin
- Difficulty M; exact reproduction: M
- Why: Zonal generalisation tools: tie-break and processing order matter for exact equality.
- Documented: nothing summarised here (only the tool page summary/parameters were parsed for these tools; any linked how-it-works page is listed under how-page in the CSV notes).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-nibble-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-aggregate-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/aggregate.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/boundary-clean.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/combine.html ...

### Image Analyst, Spatial Analyst: Fuzzy Membership, Fuzzy Overlay, Weighted Overlay, Weighted Sum, Multicriteria Overlay, Locate Regions
- Tools covered (7): Fuzzy Membership; Fuzzy Overlay; Locate Regions; Multicriteria Overlay; Weighted Overlay; Weighted Sum
- Difficulty M; exact reproduction: M
- Why: Fuzzy/weighted overlay are formula based (Y); Locate Regions is a spatially constrained optimisation whose search procedure is not specified.
- Documented: Multicriteria Overlay: two steps (transform criteria to a common scale, then weight/combine by an MCDA method). Locate Regions: selects best contiguous same-value regions meeting total area, region count, shape and distance constraints.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-multicriteria-overlay-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-locate-regions-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-fuzzy-membership-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/weighted-sum.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/fuzzy-membership.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/fuzzy-overlay.html ...

### Spatial Analyst: Generate Weighted Voronoi
- Tools covered (1): Generate Weighted Voronoi
- Difficulty L; exact reproduction: Y
- Why: Newer tool with documented influence function; higher-order allocation and polygon output ordering to be checked.
- Documented: Partitions space into regions of influence per site by lowest influence cost under a chosen Voronoi type; extends Thiessen (first-order unweighted Voronoi) with site weights, higher order and a generalised influence expression; raster allocation plus polygon output.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-generate-weighted-voronoi-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/generate-weighted-voronoi.html

### Spatial Analyst: Kernel Density, Line Density, Point Density, Calculate Kernel Density Ratio, Space Time Kernel Density, Value Percentile Contours ...
- Tools covered (8): Calculate Kernel Density Ratio; Kernel Density; Line Density; Point Density; Raster to Weighted Points; Space Time Kernel Density; Value Percentile Contours; Volume Percentile Contours
- Difficulty M; exact reproduction: Y
- Why: Kernel shapes are documented; exact normalisation, barrier option and cell-sampling details need ArcGIS comparison.
- Documented: Kernel Density: a smoothly curved surface fitted over each point, highest at the point and reaching zero at the search radius (circular neighbourhood only); volume under the surface equals the population value; line features supported.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/how-kernel-density-works.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/calculate-kernel-density-ratio.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/kernel-density.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-analyst/line-density.html ...

### Spatial Statistics: Spatial Autocorrelation (Global Moran's I), High/Low Clustering (Getis-Ord General G), Incremental Spatial Autocorrelation, Average Nearest Neighbor, Multi-Distance Spatial Cluster Analysis (Ripley's K Function), Hot Spot Analysis (Getis-Ord Gi*) ...
- Tools covered (32): Assess Sensitivity to Attribute Uncertainty; Average Nearest Neighbor; Bivariate Spatial Association (Lee's L); Calculate Composite Index; Calculate Distance Band from Neighbor Count; Calculate Rates; Central Feature; Cluster and Outlier Analysis (Anselin Local Moran's I); Collect Events; Colocation Analysis; Dimension Reduction; Directional Distribution (Standard Deviational Ellipse); Evaluate Bin Sizes for Point Aggregation; Generate Network Spatial Weights; Generate Spatial Weights Matrix; High/Low Clustering (Getis-Ord General G); Hot Spot Analysis (Getis-Ord Gi*); Hot Spot Analysis Comparison; Incremental Spatial Autocorrelation; Linear Directional Mean; Local Bivariate Relationships; Mean Center; Median Center; Multi-Distance Spatial Cluster Analysis (Ripley's K Function); Neighborhood Summary Statistics; Optimized Hot Spot Analysis; Optimized Outlier Analysis; Spatial Association Between Zones; Spatial Autocorrelation (Global Moran's I); Spatial Outlier Detection; Standard Distance; Time Series Smoothing
- Difficulty M; exact reproduction: M
- Why: Statistics are formula-defined (replicable) but permutation p-values use random numbers and neighbour definitions (spatial weights, distance band auto-selection, FDR correction) need exact ArcGIS conventions.
- Documented: Global Moran's I = (n/S0) * sum_ij w_ij z_i z_j / sum_i z_i^2 with z-score and p-value; Gi*: local sum of a feature and its neighbours compared proportionally to the global sum, z-scores/p-values with optional FDR correction for multiple testing and spatial dependence; Optimized Hot Spot Analysis runs Gi* with parameters derived from the data (aggregates incident points, picks scale of analysis from the distribution) and FDR correction.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/h-how-spatial-autocorrelation-moran-s-i-spatial-st.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/h-how-hot-spot-analysis-getis-ord-gi-spatial-stati.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/how-optimized-hot-spot-analysis-works.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/spatial-weights.html
- Tool pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/assess-sensitivity-to-attribute-uncertainty.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/average-nearest-neighbor.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/bivariate-spatial-association.html ...

## Part 3. HARD areas that are not single tools

### Esri Grid (Arc/Info binary grid)
- Why: Proprietary on-disk format; format page documents structure only conceptually (integer grids have a VAT with VALUE/COUNT; float grids have none); supports 32-bit integer and float only. GDAL reverse-engineers it; no Esri spec.
- Documented: Documented: grid = array of equally sized square cells; integer grids carry a value attribute table; Esri Grid stacks reference several grids as multiband.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/esri-grid-format.html

### CRF (Cloud Raster Format) and MRF
- Why: CRF is an Esri format (tiles in bundles, multidimensional); MRF is NASA public. Raster formats page names them but gives no layout.
- Documented: CRF: large rasters broken into bundles of tiles for parallel writing; MRF: NASA technology combining raster storage with cloud tiling/indexing and multiple resolutions.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/supported-raster-dataset-file-formats.html

### Mosaic datasets
- Why: Geodatabase structure with footprints, boundaries, seamlines, overviews, raster types and function chains; no published algorithms (see mosaic tool entries).
- Documented: see linked pages; no algorithm given
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/mosaic-datasets.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/mosaic-dataset-seamlines-pro-.html

### Raster functions (about 170, function chains, .afr/.rft.xml)
- Why: Large library evaluated lazily on pixel blocks with templates and Python raster functions; each function needs its own semantics.
- Documented: List of raster functions page enumerates them by category with license requirement.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/analysis/raster-functions/list-of-raster-functions.html ; https://doc.esri.com/en/arcgis-pro/latest/help/analysis/image-analyst/a-complete-list-of-arcgis-image-analyst-extension-raster-and-image-functions.html ; https://doc.esri.com/en/arcgis-pro/latest/help/analysis/spatial-analyst/basics/list-of-arcgis-spatial-analyst-raster-functions.html

### Geodatabase editing in place (insert/update/delete, schema change, domains, subtypes, relationship classes, attribute rules, topology, versioning)
- Why: File geodatabase system tables and GDB_Items XML must be rewritten consistently; enterprise versioning is RDBMS side; attribute rules run Arcade.
- Documented: Documented conceptually only (versioning types, topology rules and fixes, attribute rule types).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/overview-of-versioning-in-arcgis-pro.html ; https://doc.esri.com/en/arcgis-pro/latest/help/editing/geodatabase-topology-rules-for-polygon-features.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/an-overview-of-attribute-rules.html

### Arcade expression language
- Why: Separate language (ArcGIS Arcade) used in attribute rules, labeling, popups, symbology, Calculate Field, Calculate Value; needs a full interpreter and Esri function library (language spec is on developers.arcgis.com, not fetched here).
- Documented: Referenced from the attribute-rule pages and Calculate Value (ModelBuilder) which "returns a value based on a specified Python or ArcGIS Arcade expression".
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geodatabases/overview/author-attribute-rule-script-expressions.html ; https://doc.esri.com/en/arcgis-pro/latest/tool-reference/modelbuilder-toolbox/calculate-value.html

### Labeling (Maplex and Standard label engines)
- Why: Placement optimisation with many properties (position, fitting strategy, conflict resolution); needs font metrics and rendering; no published algorithm.
- Documented: Maplex tabs: Position (where the label goes), Fitting Strategy (how to make it fit in crowded areas), conflict resolution, feature-type-specific options (streets, contours, rivers, boundaries, parcels).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/mapping/text/label-with-the-maplex-label-engine.html ; https://doc.esri.com/en/arcgis-pro/latest/help/mapping/text/labeling-basics.html

### Symbology and map/layout rendering (CIM symbols, .stylx, layouts, export)
- Why: Pixel-exact rendering of marker/line/fill layers, effects, dictionary symbols; layout export to PDF/PNG.
- Documented: See Maps and scenes > Layers > Symbology and Layout help sections (not summarised).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/projects/supported-data-types-and-items.html

### Network Analyst solvers (TSP, VRP, location-allocation, hierarchical routing)
- Why: Explicitly proprietary tabu-search implementations.
- Documented: See Network Analyst tool entries.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/analysis/networks/algorithms-used-by-network-analyst.html

### Geocoding locators
- Why: Proprietary locator format and address matching.
- Documented: See Geocoding tool entries.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/data/geocoding/about-locators.html

### Deep learning / ML
- Why: Model runtimes and training; not bit-reproducible.
- Documented: See AutoDL / deep learning entries.
- Pages: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/image-analyst/how-autodl-works.html

### Proprietary file formats (DWG/DGN, Revit RVT, MrSID, ECW, Esri .gdb compressed tables, .lyrx/.aprx/.mapx CIM)
- Why: Closed or partly documented; supported-data-types page lists them.
- Documented: Supported data types and items page (create/read/write per item).
- Pages: https://doc.esri.com/en/arcgis-pro/latest/help/projects/supported-data-types-and-items.html ; https://doc.esri.com/en/arcgis-pro/latest/help/data/imagery/supported-raster-dataset-file-formats.html

## Part 4. Rows auto-flagged HARD by a text rule

- Data Management / Add Portal Items To Catalog Dataset: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/add-portal-items-to-catalog-dataset.html)
- Data Management / Upload File To Portal: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/upload-file-to-portal.html)
- Data Management / Update Portal Dataset Owner: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/data-management/update-portal-dataset-owner.html)
- Network Analyst / Share As Route Layers: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/network-analyst/share-as-route-layers.html)
- Spatial Statistics / Convert Spatial Statistics Popup Charts for Web Display: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/spatial-statistics/convert-spatial-statistics-popup-charts-for-web-display.html)
- Analysis / Enrich Layer: Online/portal/service dependent (summary mentions ArcGIS Online, Enterprise, portal or hosted services); not locally reproducible (https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/enrich-layer.html)
