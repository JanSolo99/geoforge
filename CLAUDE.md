# GeoForge — notes for Claude

- **The target is ArcGIS Pro, not QGIS.** Tool names, parameters, defaults, outputs and workflows follow the ArcGIS Pro tool reference and arcpy. When ArcGIS and QGIS/GDAL behave differently, match ArcGIS.
- GDAL is used **only in tests** to check that we read and write files correctly. It is never a runtime dependency and never the reference for how a tool should behave.

- Rust workspace; one crate per format/engine under `crates/`. Keep format crates independent of each other; they depend only on `geoforge-core`.
- Correctness oracle: GDAL 3.x is installed system-wide (`ogrinfo`, `gdalinfo`, `ogr2ogr`). When implementing a reader, write tests that compare our output with GDAL's on the same file.
- FileGDB: clean-room implementation from public reverse-engineering docs (GDAL OpenFileGDB). Never use or copy the proprietary Esri FileGDB SDK.
- Work in small, tested slices; update `docs/ROADMAP.md` checkboxes as items land.
- Before committing: `cargo fmt --all && cargo clippy --workspace --all-targets && cargo test --workspace`.

## Working conventions (established so far)
- Status and next steps: `docs/ROADMAP.md`. Format findings: `docs/formats/` (FileGDB spec corrections, ArcGIS shapefile behavior).
- Crates: `core` (geometry model, FeatureClass trait, curves), `shapefile`, `filegdb` (read + write), `geometry` (overlay/buffer on `geo`), `tools` (ArcGIS tools → `ToolOutput`), `cli` (`gf`), `testkit`.
- ArcGIS Pro 3.7 runs here under Wine (GE-Proton, `~/.wine`). The `arcgis` MCP server (`tools/arcgis-bridge/`) runs tools, Describe and arcpy code; in `arcgis_python` wrap Linux paths with `P('/path')`. Use it to settle any ArcGIS behavior question before implementing. Black-box only: never decompile Esri binaries.
- New geoprocessing tool workflow: add cases to `scripts/make-arcgis-reference.py`, run it (`uv run --with "mcp>=2" python scripts/make-arcgis-reference.py`), study the outputs (schema, row order, values), implement in `geoforge-tools`, add the `gf` command and the case to `crates/geoforge-cli/tests/arcgis_reference.rs`. Comparisons use ArcGIS's 0.001 m XY tolerance (buffers: 0.2 m / 0.01%).
- GDAL (`ogrinfo`) is the oracle for file formats only. CI has an older GDAL (3.8); tests gate on `gdal_version()` where needed.
- The shell is zsh: quote globs that may not match, and use `${=var}` to split words.
