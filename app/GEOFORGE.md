# RustGIS inside GeoForge

This directory is a copy of [opengeos/RustGIS](https://github.com/opengeos/RustGIS) (Apache-2.0,
see `LICENSE`), commit `4616893ccd13ca8fb174874aebe2b0c08e53915f`, taken 2026-10-16. It is its own Cargo workspace: the application
shell (map view, symbology, labels, layouts, tiles, web app, Python/MCP, assistant).

GeoForge's engine (`../crates`: File Geodatabase, ArcGIS-verified geoprocessing, rasters) is to be
wired in as the data and analysis backend. Changes we make here stay under Apache-2.0; keep the
original copyright notices and mark modified files.

## Wiring status
- `crates/data/src/source/filegdb.rs` (added by GeoForge): a `FeatureSource` for `.gdb` folders over
  `geoforge-filegdb` (spatial-index extent reads, curves densified, MultiPatch refused). Reached through
  `source::open_path` for the `gdb` extension; not in `EXTENSIONS`, since a geodatabase is a folder.
  Desktop only (not built for wasm32).
- `crates/geoforge` (`rustgis-geoforge`): registers `geoforge-tools` as the external suite `geoforge`
  (ids `geoforge:clip`, `erase`, `intersect`, `union`, `symmetrical_difference`, `dissolve`,
  `multipart_to_singlepart`, `feature_to_point`, `select`, `buffer`, `near`, `spatial_join`,
  `select_by_location`, `frequency`, `statistics`, `identity`, `update`, `merge`, `append`,
  `minimum_bounding_geometry`, `feature_vertices_to_points`, `points_to_line`, `sort`, `add_xy`, `add_field`,
  `delete_field`, `calculate_field`, `delete_identical`, `find_identical`, `generate_near_table`), enabled by the engine feature `geoforge`. Layers are
  WGS 84 lon/lat; topology tools run on them as they are, tools that measure (Buffer, Near, search
  distances) run in the UTM zone at the centre of their inputs (metres, planar, as ArcGIS on projected
  data) and project the result back (`Work` in `crates/geoforge/src/lib.rs`); Buffer is the exception: it is
  ArcGIS's geodesic buffer on the ellipsoid, the UTM zone only where that would cross the antimeridian or a pole. An undissolved Buffer of data wider
  than ~12° runs a UTM zone at a time (each feature in its own zone, order kept); other measuring tools on
  such data use one zone and say distances are approximate. Layers do not yet remember a native CRS. `geoforge-tools`
  has a default `proj` feature (Project); the app turns it off because proj-sys's `libsqlite3-sys`
  conflicts with the app's rusqlite.
- Output: `source::filegdb::write` / `io::write_path` write a layer as a feature class in a `.gdb` (`x.gdb`
  names it after the layer, `x.gdb/name` chooses the name; an existing class of that name is replaced; a
  folder that is not a geodatabase is refused). Field/table names are made valid, integers wider than 32
  bits become doubles, booleans small integers. `layer.addData` adds a `.gdb` (all its feature classes) or
  `x.gdb/name`; `share.exportLayer` takes `*.gdb`. ArcGIS Pro opens the output (Describe, cursor, Check
  Geometry).
- Raster tools (`crates/geoforge/src/raster.rs`): Slope, Aspect, Hillshade, Curvature, Contour, Fill, Flow
  Direction/Accumulation/Length, Basin, Watershed, Stream Link/Order, Focal Statistics, Reclassify, Zonal
  Statistics, Resample, Euclidean Distance, Extract by Mask, Plus/Minus/Times/Divide, Int, Float, Is Null, Con,
  Set Null, Raster To Point/Polygon, Point/Polyline/Polygon To Raster (cell size in degrees), Zonal Statistics as
  Table. Vector extras (`extra.rs`): Add Geometry Attributes (lengths and areas on the ground), Check Geometry,
  Repair Geometry, Split. A raster is read from its GeoTIFF (file or embedded bytes) so the pixel type
  and NoData are the file's; results are GeoTIFFs with ArcGIS's tiling and compression, returned as raster
  layers in the input's coordinate system. Vector results (contours, points, polygons) are projected to
  longitude/latitude. Absurd sizes (a contour interval giving over 100,000 levels, a cell size giving over 2^31
  cells, a neighborhood of millions of cells) are refused; the hostile-parameter test covers them.
- The CLI, Python and MCP surfaces of the app dispatch through `gp.run`, so every registered tool is
  reachable from them (`rustgis-cli run --cmd 'gp.run={"tool":"geoforge:buffer",...}'`, checked end to end
  with an export to a `.gdb`). The hostile-parameter test runs every registered GeoForge tool on junk
  parameters and fails if one panics.
- Next: Check/Repair Geometry, Split, Project (needs a PROJ-free path), Feature Class To Feature Class, and
  the raster tools in the registry.
