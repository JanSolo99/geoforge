# RustGIS inside GeoForge

This directory is a copy of [opengeos/RustGIS](https://github.com/opengeos/RustGIS) (Apache-2.0,
see `LICENSE`), commit `4616893ccd13ca8fb174874aebe2b0c08e53915f`, taken 2026-10-16. It is its own Cargo workspace: the application
shell (map view, symbology, labels, layouts, tiles, web app, Python/MCP, assistant).

GeoForge's engine (`../crates`: File Geodatabase, ArcGIS-verified geoprocessing, rasters) is to be
wired in as the data and analysis backend. Changes we make here stay under Apache-2.0; keep the
original copyright notices and mark modified files.
