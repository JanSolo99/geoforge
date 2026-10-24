# Known gaps

Where UniGIS differs from ArcGIS Pro 3.7 or leaves a documented behaviour out. Each line says what
is missing, what we do instead, and where the evidence is. Fix these before claiming a tool is done.
Update this file when a gap is found or closed.

## Differs from ArcGIS

| Tool | Gap | Evidence |
|---|---|---|
| Aggregate | Native engine currently processes the first band only; `ug aggregate` refuses multiband inputs to avoid dropping bands. Other pixel types, negative integer storage and raster environments beyond the saved float/positive-integer probes remain unverified | `scripts/research/blockops/probe.py`, `crates/unigis-cli/tests/aggregate.rs` |
| Block Statistics | Circular MAP neighborhoods require square raster cells; irregular/weighted/wedge neighborhoods are unsupported. VARIETY capacity boundaries beyond the observed 1/4/9/12/15/16/25/29/36/64/256 cases, extreme integer overflow and raster environments remain unverified | `scripts/research/blockstats/README.md`, `crates/unigis-raster/tests/blockops.rs` |
| Feature To Polygon | With several holes in one polygon side by side (one directly left of another's lowest vertex), the hole order and the vertex a hole starts at can differ (same shapes). Our upward monotone-partition sweep finds diagonals at nearly every ArcGIS entry vertex, but no walk order over them fits all 64 holes of the probe; holes next to removed bridges likewise | `scripts/research/planar/` (probes `hh`, `tri` in the notes), `crates/unigis-tools/src/planar.rs` (`place_holes`) |
| Feature To Line (polygon input) | ArcGIS sometimes splits a crossed ring once more at a vertex that is no node (the original start, or the vertex after the top-right one); we never do. No rule found on 13 rings | `scripts/research/planar/README.md` |
| Polygon To Line / Feature To Polygon / Feature To Line | Z and M are dropped (2D output); a label point lying on a line also adds a second vertex 0.004 m away in one ArcGIS run | `scripts/research/planar/README.md` |
| Median Center | Weiszfeld from the mean center matches ArcGIS step for step, but ArcGIS stops after a different step (no step-size, objective or scale rule fits 12 cases). Results are up to ~10 m apart | `scripts/research/spatstat/`, `testdata/arcgis-reference/spatstat.json` (tolerance 10 m in `crates/unigis-cli/tests/spatstat.rs`) |
| Near / Generate Near Table (geodesic) | Point to line is up to 0.08 % off (ArcGIS treats segments as straight in lon/lat); pairs across the antimeridian are not handled | `docs/ROADMAP.md` (geodesic Near) |
| Select By Location | Distances still use the UTM zone, not geodesic | ROADMAP |
| Linear Directional Mean | Orientation-only mean is matched on 4 runs; weights and multi-vertex lines (start-to-end chord used) are not checked | `testdata/arcgis-reference/spatstat.json` |
| Spatial Statistics in the app | Planar, in the data's UTM zone: wide data (a continent) fails with "could not be projected back"; ArcGIS uses geodesic methods for geographic data | `app/apps/rustgis-python/tests/test_arcpy.py` (uses one country) |
| Flow Direction on flats | 5x5 look-ahead approximation; a few float-DEM cells and ~12 % of an integer DEM's flats differ | ROADMAP |
| Raster To Polygon SIMPLIFY | Corner choice on smooth curves not reproduced (104 of 122 blocky test rasters exact) | `docs/formats/raster-to-polygon-simplify.md` |
| IDW | No barriers; the default cell size is checked only with the points' extent | `crates/unigis-raster/tests/idw.rs` |
| Integer NoData of math functions | S32 results of Square/Int use NoData -2147483647 (ArcGIS), but `Int` and the older `Plus`/`Times` S32 outputs still use -2147483648; unverified which ArcGIS gives for them | `crates/unigis-raster/tests/mathfn.rs` |
| Integer output types of overlay tools | The `0..1` range gives NoData 16 and wide signed input stays S32 (observed on few runs) | `crates/unigis-raster/tests/overlay.rs` |

| Dice (polygons) | Lines match on 4 probe sets. Polygons match when the cells hold more than ~20 vertices (cut at the upper median vertex, across the longer side, upper half first, part by part). With smaller cells (11 to 18 vertices, limits near 10 to 33) ArcGIS cuts across the shorter side in about 10 of 41 cuts, and no rule on extents, spread or alternation explains it; the tie between a nearly square cell is decided by an unknown threshold (0.116 apart cut across x, 0.163 across y; we use 0.125). A neck of zero width keeps one ring in ArcGIS, two parts here | `scripts/research/unsplit/` (probes), `testdata/arcgis-reference/dice_*`, `KNOWN_FEATURE_DIFFERENCES` in `crates/unigis-cli/tests/arcgis_reference.rs` |
| Zonal Histogram, Tabulate Area, Zonal Geometry | Zonal Histogram and Tabulate Area match on 12 cases (same-grid and coarser value rasters exact). When the value raster has finer cells than the zones, ArcGIS reads it at an unknown point of each cell: one rule (left edge, three quarters down) fits the single pair tried. Zonal Geometry (as table) is not done: area, perimeter (cell edges, the raster border counts), centroid (6 significant digits), and the ellipse axes are understood, but THICKNESS (15 = half a cell, 21.21 = 15 x 1.414, 238/138 for a 240 x 280 block touching the border) has no rule yet | `crates/unigis-raster/src/zonal_tables.rs`, `scripts/research/blockstats/`, cases `zh_*`, `ta_*`, `zgt_*` (skipped in the test) |
| Cost Back Link | Where equally cheap paths meet, ArcGIS prefers a diagonal predecessor in most cases but not all, and picks either of two straight ones depending on the cell; 15 of 2000 cells differ on a random integer cost raster (distances and allocation are exact). Corridor is done. Source cost multiplier, start cost, resistance rate, capacity, Cost Path (probed: `cpath_*` cases, skipped in the test; path cells carry numbers 3, 4, 5 for destinations 2, 1, 3, sources 1) and Path Distance are not done | `crates/unigis-raster/src/cost.rs`, `scripts/research/blockstats/cost_ties.py`, case `cback_int` |
| Pattern statistics | ArcGIS's p-values come from an approximation up to 2e-8 from the exact normal tail; inverse distance treats distances under 1 as 1 (documented by Esri, not exercised by the references); the false discovery rate bins are checked only where nothing is significant | `crates/unigis-cli/tests/spatstat_patterns.rs` |
| Generate Spatial Weights Matrix | Equidistant neighbors (regular grids): ArcGIS picks and orders tied K nearest neighbors (and the nearest-neighbor fill of distance bands) in an order no tested rule reproduces (std/numpy sorts, heaps, k-d tree queries of its own tree); cocircular points (grids) make the Delaunay neighbors depend on ArcGIS's triangulator. The neighbor order inside Delaunay and contiguity rows follows a Python set whose insertion order is unknown, so large inputs list the same neighbors in another order (weights and statistics are unaffected). Polygon centroids can differ from ArcGIS's in the last bit (weights ~1e-12 apart). Generate Network Spatial Weights is not done (no network datasets) | `scripts/research/spatstat/weights/`, `crates/unigis-cli/tests/spatstat_weights.rs` (`TIES`, `ordered`) |
| Cluster and Outlier Analysis | Permutation p-values and z-scores are random in ArcGIS too (two runs differ); ours use a fixed seed. Tests use 0 permutations | `scripts/research/spatstat/weights/probe_p8.py` (`p99`, `p99b`) |
| Multi-Distance Spatial Cluster Analysis | Ripley's edge correction matches while no circle around a feature reaches a corner of the study area; beyond that ArcGIS weighs pairs more than the exact share of the circle inside (no corner formula tried fits). SIMULATE_OUTER_BOUNDARY_VALUES and REDUCE_ANALYSIS_AREA are not done (mirroring across the rectangle's edges and shrinking it by the distance do not fit ArcGIS's values); confidence envelopes are random in ArcGIS too | `testdata/arcgis-reference/spatstat_ripley.json` (`exact_rows`, `big_sim`, `big_red`), `scripts/research/spatstat/weights/probe_ripley.py` |
| Ordinary Least Squares | ArcGIS's t, F and chi-squared p-values come from approximations (up to 1.5e-6 apart, e.g. 0.019377 vs exact 0.019378; tiny ones like 1e-16 vs 6e-18); ours are exact. The PDF report and the app's two tables (only messages) are not done | `testdata/arcgis-reference/spatstat_ols.json`, `crates/unigis-cli/tests/spatstat_ols.rs` |
| File geodatabase editing | Freed row space is not put on a `.freelist` (ArcGIS reuses it; the file grows until Compact) and the `timestamps` file is not updated (an ArcGIS session that has the geodatabase open may not notice an edit until it is reopened). The rollback journal (`.gf-journal`) is UniGIS's: after a crash, open the geodatabase once with `ug` (any read) before ArcGIS, or ArcGIS may see a half-applied edit. ArcGIS's own `.lock` files are not honoured. Version 2 (64-bit OBJECTID) index files are checked against ArcGIS's only as one-page trees | `crates/unigis-filegdb/tests/crash_safety.rs`, `edit_arcgis.rs` |

## Not implemented

| Tool | Missing |
|---|---|
| Desktop pane layout | Floating windows and arbitrary split groups are not implemented; the attribute table remains a separate bottom view. Panes support left/right/bottom tab groups, drag/drop, auto-hide and saved arrangements (`app/crates/ui-egui/src/dock.rs`) |
| Create Fishnet | Labels output, corner with zero rows/columns |
| Densify | ANGLE and OFFSET methods, curved segments, Z/M |
| Generate Points Along Lines | Chainage fields |
| Slice, Region Group, Boundary Clean, Thin | Slice: equal area differs on a cell, natural breaks/standard deviation/geometrical interval not done; Region Group: no LINK attribute table; Boundary Clean and Thin not implemented |
| File geodatabase | Append to a non-spatial table (`ug append` into a `.gdb` table: "appending to tables" in `unigis-tools`); creating a new geodatabase is not one transaction (a crash leaves a partial `.gdb`) |
| Weighted Overlay | RESTRICTED, value fields other than VALUE, the attribute tables of Combine and Weighted Overlay, `ug weighted-overlay` |
| Multiple Ring Buffer, Create Thiessen Polygons | Probed against ArcGIS (`scripts/research/construct/probe1.py`); ring and cell ordering not analysed yet |
| Spatial Statistics | 48 of 63 tools (GWR, Incremental Spatial Autocorrelation, Generate Network Spatial Weights, ...) |
| Formats | LAS/LAZ, NetCDF/HDF, KML, DXF/DWG, Esri Grid/IMG, TIN, mobile geodatabase, `.shp.xml`, compressed `.cdf` |
| Network Analyst, 3D Analyst, Geostatistical Analyst, Cartography | Not started |

## Process gaps

- The app (RustGIS shell) has smoke tests that run tools but compare nothing with ArcGIS; the engine
  crates carry the ArcGIS comparisons.
- Rust speed is claimed, not measured: no benchmarks against ArcGIS or GDAL yet.
- The app has never been launched as a window in a test environment (`xvfb-run` missing); screenshots
  come from the offscreen harness `app/crates/ui-egui/examples/ui_shot.rs`.
