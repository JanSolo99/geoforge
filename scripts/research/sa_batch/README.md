# Registry batch findings (2026-10-26/27)

Preserved black-box ArcGIS observations from the previous handoff. Implementation
status here is historical at `526a0f2`; consult `ug tools`, the roadmap and known
gaps for current support. This is research context, not the active task list.

## ArcGIS behavior and implementation limits

- Added Polygon Neighbors (`polygon_neighbors`, table out: src_/nbr_ field pairs, [AREA], LENGTH, NODE_COUNT; BOTH_SIDES only)
  and Count Overlapping Features (`count_overlapping_features`, single input only; rows ordered by count ascending) in
  `crates/unigis-tools/src/neighbors.rs`; cases `pn_squares*`, `cof_clip` pass `arcgis_reference`.
  ArcGIS rejects two inputs of unequal shape type (`cof_squares_clip` failed, removed).
- Subdivide Polygon (`subdivide_polygon`, NUMBER_OF_EQUAL_PARTS and EQUAL_AREAS only) implemented and passing `sbd_*`.
  Probed rule (corrects the earlier guess): ArcGIS always cuts with lines parallel to the X axis, from the bottom up, into
  equal-area strips whatever the polygon's shape or rotation; one feature per strip (multipart if split); EQUAL_AREAS gives
  ceil(area/target) strips, the top one the remainder. Other methods (STACKED_BLOCKS, split angle, width) not done.
- Tabulate Intersection (`tabulate_intersection`, in `neighbors.rs`; cases `ti_*`) implemented: zone features grouped and dissolved
  by zone fields, class features grouped by class fields but NOT dissolved (overlapping class features count twice, observed
  with no class fields); rows sorted by zone then class values, zero intersections omitted; AREA (or LENGTH for line classes)
  plus PERCENTAGE = amount / zone group area (polygon classes) or / total class length (line classes: observed, the zone's
  share rule for lines is a guess). Not done: sum_fields, xy_tolerance, point classes. ArcGIS measures on its coordinate grid:
  the test compares `ti_` tables to 2e-5 relative / 1e-3 absolute.
- Create Thiessen Polygons (`create_thiessen_polygons`, cases `th_*`, passing): Voronoi cells (half-plane clipping, O(n^2)) inside the
  points' extent grown by 4.9945% of width/height per side (observed on 3 datasets); fields: default `Id`, `Input_FID` (input FID),
  with ALL the input fields (`Id` -> `Id_1`). ArcGIS lists the polygons in an internal spatial order that is NOT reproduced (we list
  input order; the test sorts `th_` cases by attributes). Not done: the Extent environment, duplicate points, geodesic/GCS input.
- Simplify Line POINT_REMOVE (`simplify_line`, cases `sl_*`, passing): same vertices as plain Douglas-Peucker (dist > tol keeps);
  output adds InLine_FID, MaxSimpTol, MinSimpTol (= tolerance). ArcGIS also writes `<out>_Pnt.shp` (collapsed points; we do not write it: empty in
  the probes). Not done: BEND_SIMPLIFY, polygons (Simplify Polygon), barriers, error resolving (topology repair).
- Spatial Analyst single-raster Math tools (25: Abs, Negate, Square, Square Root, Exp/Exp2/Exp10, Ln/Log2/Log10, Sin..ATanH, Round Down/Up,
  Boolean Not) as `unary` in `unigis-raster/src/local.rs` + `unary_tool!` entries in the raster registry; 75 cases `um_*` (3 inputs each)
  all pass. Rules: float functions give F32 with NoData where not finite as f32; integer Abs -> unsigned same width; Negate -> signed
  (U8/S16 -> S16); Square U8 -> U16, S16 -> S32 with NoData -2147483647 (not i32::MIN); Round Down/Up always F32; Boolean Not U8 NoData 2.
  Not probed: S32/U16 inputs of Abs/Negate/Square. Next in this family: Power, Mod, Boolean And/Or/Xor, comparison tools, Bitwise.
- Two-operand Math tools (16: Power, Mod, Boolean And/Or/XOr, Equal To, Greater/Less Than (Equal), Not Equal, Bitwise And/Or/XOr/Left/Right
  Shift) as `binary` in `local.rs`, operands are rasters or constants (`in1`, `in2`); 128 cases `bm_*` (8 operand pairs each) pass.
  Rules: Power F32 (non-finite -> NoData); comparisons/Boolean U8 NoData 255 (nonzero true); Bitwise truncates to int, S32, shift count
  mod 32; Mod = C remainder (sign of dividend), float if an operand is, integer type from the operands' stored statistics when both known
  (`mod_type`, constant dividend: divisor alone sets the range), else the wider type (signed if the dividend raster or divisor raster is).
  Not probed: negative-valued stats inputs for Mod beyond Negate, U16/S32 operands, Mod type when one operand lacks statistics but is
  signed 32-bit. Next: Combinatorial And/Or/Xor, Zonal Fill, Raster To ASCII/Float, Copy Raster, Mosaic, Reclass by ASCII file.
- Raster To ASCII (`raster_to_ascii`, `conversion::to_ascii`; cases `r2a_*`, byte-for-byte, pass): header names padded to 14, NoData
  always -9999, rows end with a space, CR LF line ends, floats `%.7g` of the f32 value. `ug run` special-cases this tool to write
  text (see `run_raster_tool`). The `.prj` (Esri `Projection UTM / Zone 33 / Datum WGS84 ...` block) and `.asc.xml` sidecars are NOT
  written. Raster To Float (`.flt` + `.hdr` + `.prj`) not done. Combinatorial And/Or/XOr: see the next entry.
- Combinatorial And/Or/XOr (`combinatorial`, cases `cb_*`, 9, pass): each distinct (a,b) pair gets the next number in first-occurrence
  (row-major) order; pairs the operation calls false get 0 and use no number (And: either 0; Or: both 0; XOr: both or neither 0);
  NoData if either input is; type = `integer_output(0, max id)` (4-bit NoData 15 for small ids, S8 -128 up to 127, ...). Probed only on
  same-grid integer rasters; different grids go through `common_grid` untested.
- ATan2 (F32, both-zero -> NoData, args are y then x) and Bitwise Not (S32, -trunc(x)-1) added; cases `bm_atan2_*`, `um_bitwise_not_*` pass.
  Dashboard scanner now also reads macro-declared tools (`unary_tool!`/`binary_tool!`) and `neighbors.rs`.
  Probed but NOT done: Equal To / Greater Than / Less Than Frequency (U8; NoData 3 for 2 rasters but 16 for 3 rasters: rule unclear),
  In List. Remaining sa gaps are listed by the dashboard (`python3 scripts/dashboard/build.py`, page "missing").
- Table Select (`table_select` = Copy Rows with an SQL where, same 19-wide decimal cap; cases `tsel_*`) passes.
- Export Features = alias of Feature Class To Feature Class (where clause, field mapping; sort_field falls through to an error); cases `ef_*`.
- Generate Transects Along Lines (`generate_transects_along_lines`, `gt_*` pass): every `interval` along the line (end points listed last), 2-point line of the given length, normal = left normal flipped to point up (y>=0), first vertex on the lower side; fields Id, ORIG_FID. Probed not done: Generate Rectangles Along Lines (fields PageNumber, GroupId, SeqId, Previous, Next, Angle), Grid Index Features (PageName, PageNumber).
- Grid Index Features (`grid_index_features`, `gix_*` pass; planar width/height, no page units/origin/rows/cols options): grid anchored at the extent's lower-left, rows from the top named A.., columns 1.., INTERSECTFEATURE keeps cells touching a feature (closed box, so touching counts) and renumbers.
- Reclass by ASCII File (`reclass_by_ascii_file`, `rba_*` pass; remap files `testdata/raster/remap_ascii{1,2}.txt`): lines `v : new` / `from to : new`; a `NODATA` new value reads as 0 in ArcGIS (observed); type rules as Reclassify, but with NODATA missing values a float input also gets the 8-bit output rules. Format with dashes (`188 - 220 : 1`) gave all-NoData in ArcGIS: not reproduced. Probed, NOT done: Simplify Polygon (POINT_REMOVE is not plain Douglas-Peucker: more vertices appear, areas change).
- Edit toolbox: Flip Line (`flip_line`) and Generalize (`generalize`, lines only, Douglas-Peucker = ArcGIS vertices) added; cases `flip_roads|streams`, `gen_*` pass. They run as IN_PLACE copies in the reference script. Probed usage only, NOT done: Extend Line (length, EXTENSION|FEATURE), Trim Line (dangle_length, DELETE_SHORT|KEEP_SHORT), Snap, Erase Point.
- Summarize Within (`summarize_within`; `sw_parcels_wells*` pass): ArcGIS writes it only to a geodatabase or memory, so the reference script's new `MEMORY_OUT` set runs such tools into `memory/` and copies the result to a shapefile (reuse it for Summarize Nearby, Enrich-like tools). Output = input fields + `<stat>_<FIELD>` + `Point_Count|Polyline_Count|Polygon_Count`; KEEP_ALL lists polygons with features first, then empty ones. ADD_SHAPE_SUM (length/area inside) is refused: ArcGIS measures it geodesically (a UTM line is 0.04 % longer than its planar length: 1219.769 vs 1219.282), which needs the projection engine in unigis-tools.
- Raster To Float (`raster_to_float`, `r2f_*` byte-exact .flt pass): LE f32, NoData -9999, plus `.hdr` (ASCII header + `byteorder     LSBFIRST`, CRLF); `.hdr`/`.prj`/`.xml` sidecars are not compared (.prj not written). The reference test now compares `.asc`/`.flt` as bytes.
- Copy Raster (`copy_raster`, `cpr_*` pass; TIFF only; options nodata_value, pixel_type): default NoData = input's, else 3.4e38 for float / type max for ints; statistics sidecar only when the input has no NoData cells. Quirk NOT reproduced (case dropped): float -> 16-bit signed keeps the NoData 3.4e38 as the tag text. Mosaic / Mosaic To New Raster not done.
- Mosaic To New Raster (`mosaic_to_new_raster`, `mos_*` pass; same-cell-size rasters, methods FIRST/LAST/MEAN/MINIMUM/MAXIMUM, no BLEND, no colormaps): union extent snapped to the FIRST raster's grid (cell = first's unless given); NoData tag = max input NoData + 1 (e.g. 256 for U8, unreachable) or 3.4e38 for float inputs without NoData; cells without data are 0 in integer output, NoData in float output. Different cell sizes/resampling and other input pixel types untested.
- Extract by Polygon / Extract by Points (`extract_by_polygon|points`, coordinates as `x y;x y;...`, `ebpoly_*`/`ebpts_*` pass): input grid kept, statistics written; polygon test is scanline-style (span includes its right end, not its left: edge cells belong on right edges only); a point on a cell edge goes to the cell right of / above it. Feature-set polygons (shapefile) not wired.
- Topographic Position Index (`tpi_*`) and Test (`test_*`) added, all pass. TPI: z minus the mean of the in-raster cells with data of the neighborhood (default: circle radius 3 WITHOUT the center; an explicit Circle includes it); F32, NoData +3.4028235e38. Test: U8 0/1, NoData 2.
- Top Hat Transform (`top_hat_transform`, `tophat_*` pass): RIDGE = z - opening, VALLEY = closing - z over the neighborhood (default circle radius 3, ridge), outside-raster cells ignored, intermediate keeps input NoData; annulus is refused with ERROR 000864 as ArcGIS does.
- Trend LINEAR (`trend`, `trend_wells_*` pass for orders 1 and 2 and the default): least-squares polynomial on IDW's default grid (half a cell below the lower-left point, round(w/cell)+1 columns). Order 3 differs a lot (ArcGIS ~79 vs ours ~56 at the corner): probably ArcGIS solves the raw-coordinate normal equations (ill-conditioned) and its result is numerically poor; not matched (case dropped). LOGISTIC and the RMS file are not done. The raster registry `Context` got `point_samples(name, field)` for tools that read point features.
- Rank and Popularity (`rank`, `popularity`; constant rank k + semicolon list of same-grid rasters; cases `rk_*`, `pop_*` pass): Rank = k-th smallest; Popularity = k-th most frequent distinct value, NoData on ties at that rank or when k exceeds the distinct values, except that when all rasters agree every k gives that value. Type = widest of the inputs, floating if any is, signed only if a signed raster's statistics have negatives (a S16 file with all-positive stats gives U16). Rank given as a raster is not supported.
- In List (`in_list`, `inl_*` pass): keeps the cell value where it equals a constant of the list or that cell's value in a list raster, else NoData; input type kept, NoData = the type's default. Frequency tools (Equal To/Greater Than/Less Than Frequency): counts verified (U8) but the NoData value (16, 3, 16, 15 for 1..4 rasters) follows no rule found; not done.
- Create Constant Raster (`create_constant_raster`, 13 cases `cc_*` pass): grid = extent / cell size, columns and rows rounded; integer output takes the smallest type (1/2/4-bit layouts with their NoData, S8, S16, U16; S32 with NoData -2147483647 beyond that); float is F32. No coordinate system is attached.
- Summarize Nearby (STRAIGHT_LINE, POINT summary features only; `sn_parcels_wells*` pass with a looser buffer tolerance 0.25 m / 1.5e-3 because its corner arcs are coarser than Buffer's): output = buffer of each feature per distance (feature-major), fields input + BUFF_DIST + Point_Count, KEEP_ALL default with features that have summary features first. NOT matched: several distances (rings; counts differ by 1 near arcs; case dropped) and line/polygon summary features (geodesic shape sums added by ArcGIS). CAUTION learnt: never `git checkout` a file with uncommitted work (lost and rewrote summarize_nearby once).
