# Prior art: RustGIS (opengeos/RustGIS)

Looked at 2026-10-16 (repo created 2026-10-10, Apache-2.0, ~122k lines of Rust, 1.2k stars).
We write our own code; this records what is different and what is worth adopting as an idea.

## What it is
A QGIS-style desktop/web/CLI GIS with an ArcGIS-like ribbon, built on `geo`, egui and
tiny-skia. Layers: geom, data, map/analysis, render, engine (session, undo, command registry),
assistant (LLM agent), ui-egui. Apps: desktop, CLI, WebAssembly, PyO3 Python package, MCP server.

## How it differs from GeoForge
- **No File Geodatabase at all** (no `.gdb` read or write). Shapefile/GeoPackage/GeoParquet/etc.
  Formats GDAL knows are reached by shelling out to GDAL programs when installed.
- **Thin native geoprocessing**: `crates/analysis` is ~3k lines (about 15 tools: Buffer, Clip,
  Erase, Dissolve, Feature To Point, Simplify, Summarize Within...). Its roadmap lists spatial
  join, intersect, union, near as *future*. The "1,000+ tools" are the separate GeoLibre/Whitebox
  programs, run as external processes, with their own names and semantics, not ArcGIS's.
- **No ArcGIS-fidelity goal**: no reference tests against ArcGIS output; ArcGIS parameter names
  and defaults are not followed (`distance` is in km, etc.).
- Strong where we are weak: map display, symbology, labels, layouts, tiles, web app, plugins, AI.

So the two projects overlap little: ours is the geoprocessing/data-fidelity engine, theirs is the
application shell. Nothing stops us reusing the idea of a shell later.

## Ideas worth taking (blocker 2: tool framework)
1. **One registry of commands/tools with ids, typed parameter specs, and a `mutates` flag**; the
   GUI, CLI, Python and agents all dispatch through it. Ours should add what ArcGIS needs
   (parameter direction, data types, dependencies, environments, messages, history).
2. **Hostile-parameter test**: run every tool with junk parameters and require no panic. Cheap,
   catches crashes. Add to `unigis-tools` once the registry exists.
3. **No-panic policy** (`deny(unwrap_used)` in non-test code, `unsafe_code = "forbid"`).
4. **Layering check** in CI (`cargo xtask layers`) to enforce crate dependency rules.
5. **MCP server exposing the tools** with built-in docs, and a PyO3 package (`maturin`) over the
   same session: a model for our Python bindings (arcpy-shaped API over the registry).
6. Raster CRS lookup from the `crs-definitions` crate (CC0) as a pure-Rust fallback to PROJ.

## Not to do
Do not copy its code or text (Apache-2.0 needs attribution and it would blur our clean-room
position on ArcGIS behaviour); do not adopt its tool semantics.
