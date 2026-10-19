# Known gaps

Where GeoForge differs from ArcGIS Pro 3.7 or leaves a documented behaviour out. Each line says what
is missing, what we do instead, and where the evidence is. Fix these before claiming a tool is done.
Update this file when a gap is found or closed.

## Differs from ArcGIS

| Tool | Gap | Evidence |
|---|---|---|
| Median Center | Weiszfeld from the mean center matches ArcGIS step for step, but ArcGIS stops after a different step (no step-size, objective or scale rule fits 12 cases). Results are up to ~10 m apart | `scripts/research/spatstat/`, `testdata/arcgis-reference/spatstat.json` (tolerance 10 m in `crates/geoforge-cli/tests/spatstat.rs`) |
| Near / Generate Near Table (geodesic) | Point to line is up to 0.08 % off (ArcGIS treats segments as straight in lon/lat); pairs across the antimeridian are not handled | `docs/ROADMAP.md` (geodesic Near) |
| Select By Location | Distances still use the UTM zone, not geodesic | ROADMAP |
| Linear Directional Mean | Orientation-only mean is matched on 4 runs; weights and multi-vertex lines (start-to-end chord used) are not checked | `testdata/arcgis-reference/spatstat.json` |
| Spatial Statistics in the app | Planar, in the data's UTM zone: wide data (a continent) fails with "could not be projected back"; ArcGIS uses geodesic methods for geographic data | `app/apps/rustgis-python/tests/test_arcpy.py` (uses one country) |
| Flow Direction on flats | 5x5 look-ahead approximation; a few float-DEM cells and ~12 % of an integer DEM's flats differ | ROADMAP |
| Raster To Polygon SIMPLIFY | Corner choice on smooth curves not reproduced (104 of 122 blocky test rasters exact) | `docs/formats/raster-to-polygon-simplify.md` |
| IDW | No barriers; the default cell size is checked only with the points' extent | `crates/geoforge-raster/tests/idw.rs` |
| Integer NoData of math functions | S32 results of Square/Int use NoData -2147483647 (ArcGIS), but `Int` and the older `Plus`/`Times` S32 outputs still use -2147483648; unverified which ArcGIS gives for them | `crates/geoforge-raster/tests/mathfn.rs` |
| Integer output types of overlay tools | The `0..1` range gives NoData 16 and wide signed input stays S32 (observed on few runs) | `crates/geoforge-raster/tests/overlay.rs` |

| Pattern statistics | ArcGIS's p-values come from an approximation up to 2e-8 from the exact normal tail; inverse distance treats distances under 1 as 1 (documented by Esri, not exercised by the references); the false discovery rate bins are checked only where nothing is significant | `crates/geoforge-cli/tests/spatstat_patterns.rs` |
| File geodatabase editing | Freed row space is not put on a `.freelist` (ArcGIS reuses it; the file grows until Compact) and the `timestamps` file is not updated (an ArcGIS session that has the geodatabase open may not notice an edit until it is reopened). The rollback journal (`.gf-journal`) is GeoForge's: after a crash, open the geodatabase once with `gf` (any read) before ArcGIS, or ArcGIS may see a half-applied edit. ArcGIS's own `.lock` files are not honoured. Version 2 (64-bit OBJECTID) index files are checked against ArcGIS's only as one-page trees | `crates/geoforge-filegdb/tests/crash_safety.rs`, `edit_arcgis.rs` |

## Not implemented

| Tool | Missing |
|---|---|
| Polygon To Line | `IDENTIFY_NEIGHBORS` (LEFT_FID / RIGHT_FID, shared boundaries split at nodes): needs a planar graph |
| Create Fishnet | Labels output, corner with zero rows/columns |
| Densify | ANGLE and OFFSET methods, curved segments, Z/M |
| Generate Points Along Lines | Chainage fields |
| Slice, Region Group, Boundary Clean, Thin | Slice: equal area differs on a cell, natural breaks/standard deviation/geometrical interval not done; Region Group: no LINK attribute table; Boundary Clean and Thin not implemented |
| File geodatabase | Append to a non-spatial table (`gf append` into a `.gdb` table: "appending to tables" in `geoforge-tools`); creating a new geodatabase is not one transaction (a crash leaves a partial `.gdb`) |
| Weighted Overlay | RESTRICTED, value fields other than VALUE, the attribute tables of Combine and Weighted Overlay, `gf weighted-overlay` |
| Multiple Ring Buffer, Create Thiessen Polygons | Probed against ArcGIS (`scripts/research/construct/probe1.py`); ring and cell ordering not analysed yet |
| Spatial Statistics | 54 of 63 tools (Generate Spatial Weights Matrix, Collect Events, Ripley's K, Cluster and Outlier Analysis, OLS, GWR, ...); polygon contiguity conceptualizations and weights files in the three pattern tools |
| Formats | LAS/LAZ, NetCDF/HDF, KML, DXF/DWG, Esri Grid/IMG, TIN, mobile geodatabase, `.shp.xml`, compressed `.cdf` |
| Network Analyst, 3D Analyst, Geostatistical Analyst, Cartography | Not started |

## Process gaps

- The app (RustGIS shell) has smoke tests that run tools but compare nothing with ArcGIS; the engine
  crates carry the ArcGIS comparisons.
- Rust speed is claimed, not measured: no benchmarks against ArcGIS or GDAL yet.
- The app has never been launched as a window in a test environment (`xvfb-run` missing); screenshots
  come from the offscreen harness `app/crates/ui-egui/examples/ui_shot.rs`.
