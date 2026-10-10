# GeoForge — notes for Claude

- Rust workspace; one crate per format/engine under `crates/`. Keep format crates independent of each other; they depend only on `geoforge-core`.
- Correctness oracle: GDAL 3.x is installed system-wide (`ogrinfo`, `gdalinfo`, `ogr2ogr`). When implementing a reader, write tests that compare our output with GDAL's on the same file.
- FileGDB: clean-room implementation from public reverse-engineering docs (GDAL OpenFileGDB). Never use or copy the proprietary Esri FileGDB SDK.
- Work in small, tested slices; update `docs/ROADMAP.md` checkboxes as items land.
- Before committing: `cargo fmt --all && cargo clippy --workspace --all-targets && cargo test --workspace`.
