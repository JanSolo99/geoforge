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
  `select_by_location`, `frequency`, `statistics`), enabled by the engine feature `geoforge`. Layers are
  WGS 84 lon/lat; topology tools run on them as they are, tools that measure (Buffer, Near, search
  distances) run in the UTM zone at the centre of their inputs (metres, planar, as ArcGIS on projected
  data) and project the result back (`Work` in `crates/geoforge/src/lib.rs`). Data wider than ~12°
  gets a message that distances are approximate. Layers do not yet remember a native CRS. `geoforge-tools`
  has a default `proj` feature (Project); the app turns it off because proj-sys's `libsqlite3-sys`
  conflicts with the app's rusqlite.
- Next: more tools in the registry, then Python/MCP bindings.
