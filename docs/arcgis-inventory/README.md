# Verified ArcGIS Pro tool and format inventory

Source: doc.esri.com/en/arcgis-pro/latest (the host pro.arcgis.com redirects there with HTTP 301). Retrieved 2026-10-15. Documentation version: "latest" (3.7 per the project notes).

## Method (what was actually done)
- The tool-reference table of contents (`tool-reference/toc.json`, 44 toolboxes) was downloaded and walked; every toolbox, toolset, tool, overview and concept page in it was fetched with curl (HTTP 200). The first three pages were fetched with the WebFetch tool; the rest with curl because 2,800+ pages were needed.
- Each tool page was parsed for: title, summary, Python signature (arcpy function name), and the "Licensing information" block. Nothing was filled in from memory.
- The TOC omits deprecated tools. Each toolbox "history" page lists every tool including deprecated ones; names found there but missing from the TOC were fetched by URL guess (slug of the name). 130 of 141 resolved (3 of the 144 history-only names, Calculate Value, Select Data and Merge Branch, turned out to be ModelBuilder tools); 11 did not resolve (listed below). 34 ModelBuilder-toolbox tools (not in the TOC) come from `modelbuilder-toolbox/an-overview-of-the-modelbuilder-toolbox.html`.
- arcpy API: `arcpy/toc.json` walked; all 1,049 pages fetched (functions, classes, arcpy.da, arcpy.mp, arcpy.sa, arcpy.ia, arcpy.nax, arcpy.geocoding, ...).
- File formats and raster-function lists: the help pages "Supported data types and items", "Raster file formats", "List of raster functions" and the Image Analyst / Spatial Analyst function lists.
- GeoForge status: from `ug --help` (82 commands) and docs/ROADMAP.md. A tool is "done" if a gf command exists and ROADMAP says it matches ArcGIS; "partial" if the command exists with known gaps; otherwise "missing". Status was not re-run against ArcGIS here.
- Utility Network: 67 of its 122 rows are diagram tools named in the Utility Network toolbox history page; the TOC lists tools with the same names under Network Diagram, so those rows probably duplicate Network Diagram entries (not cross-checked).
- Difficulty (S/M/L/XL), exact_repro and priority are heuristics: curated for the entries in `hard_items.md`, rule-based (toolset and name patterns) elsewhere. The column `difficulty_basis` says which. They are estimates, not documentation facts. Domain toolboxes (P4: Business Analyst, Aviation, Utility Network, ...) were not assessed in depth.
- The `arcpy_name` for Spatial Analyst / Image Analyst pages is the bare function name shown on the page; the module (`arcpy.sa` / `arcpy.ia`) was taken from the example code on the page or, where absent, inferred from the toolbox (marked in notes).

## Pages fetched
- tool-reference: 3013 HTML files (TOC pages + history-found + ModelBuilder); arcpy reference: 1049; help pages: 18; `toc.json` x3.
- Tool-reference pages that could not be fetched (404 by slug guess; names come from toolbox history pages): Defense/Generate Range Rings From Lookup Table; Location Referencing/Create UPDM Geodatabase; Network Analyst/Add Fleet Problem Routes; Parcel/Build Parcel Fabrics; Parcel/Validate Parcel Fabric; Public Transit/Connect Network Dataset Transit Sources To Streets; Space Time Pattern Mining/Visualize the Space Time Cube in 2D; Space Time Pattern Mining/Visualize the Space Time Cube in 3D; Topographic Production/Update Property Count; Utility Network/Export Trace Configuration; Utility Network/Import Trace Configuration.
- Not fetched at all: the Arcade language reference (developers.arcgis.com), other external papers cited by Esri pages, the "Learn more" pages for Geostatistical Analyst beyond those listed in hard_items.md, Topographic Production / Aviation / Defense / Maritime / Utility Network concept pages (listed in concept_pages.csv but not summarised).
- Not covered because they are not in the tool-reference TOC and no page was found: a Metadata toolbox (no metadata geoprocessing tools were found; see the arcpy Metadata module in arcpy_api.csv).

## Counts

Tool rows per toolbox CSV (a tool listed in two toolboxes, e.g. Slope in Spatial Analyst and 3D Analyst, is counted in both). "TOC" = listed in the tool-reference table of contents; "extra" = found via history page / ModelBuilder overview; "dep" = page says deprecated.

| Toolbox | rows | TOC | extra | dep | done | partial | missing | HARD (Y) |
|---|---|---|---|---|---|---|---|---|
| Data Management | 441 | 435 | 6 | 4 | 17 | 8 | 416 | 175 |
| Spatial Analyst | 237 | 236 | 1 | 1 | 24 | 5 | 208 | 100 |
| Image Analyst | 153 | 152 | 1 | 1 | 12 | 0 | 141 | 77 |
| 3D Analyst | 145 | 145 | 0 | 0 | 11 | 2 | 132 | 96 |
| Utility Network | 122 | 55 | 67 | 0 | 0 | 0 | 122 | 0 |
| Network Diagram | 74 | 74 | 0 | 0 | 0 | 0 | 74 | 0 |
| Topographic Production | 70 | 55 | 15 | 0 | 0 | 0 | 70 | 0 |
| Conversion | 64 | 52 | 12 | 5 | 8 | 3 | 53 | 15 |
| Spatial Statistics | 63 | 61 | 2 | 2 | 0 | 0 | 63 | 25 |
| Aviation | 49 | 45 | 4 | 4 | 0 | 0 | 49 | 0 |
| Cartography | 49 | 49 | 0 | 0 | 0 | 0 | 49 | 36 |
| Location Referencing | 49 | 49 | 0 | 1 | 0 | 0 | 49 | 0 |
| Business Analyst | 48 | 47 | 1 | 0 | 0 | 0 | 48 | 0 |
| Network Analyst | 42 | 35 | 7 | 7 | 0 | 0 | 42 | 27 |
| Analysis | 38 | 37 | 1 | 1 | 12 | 3 | 23 | 4 |
| Geostatistical Analyst | 35 | 35 | 0 | 0 | 0 | 0 | 35 | 28 |
| ModelBuilder | 34 | 34 | 0 | 0 | 0 | 0 | 34 | 0 |
| GeoAnalytics Desktop | 31 | 31 | 0 | 0 | 0 | 0 | 31 | 0 |
| GeoAnalytics Server | 29 | 29 | 0 | 0 | 0 | 0 | 29 | 0 |
| Raster Analysis | 29 | 29 | 0 | 0 | 0 | 0 | 29 | 20 |
| Parcel | 25 | 25 | 0 | 0 | 0 | 0 | 25 | 0 |
| Indoors | 24 | 19 | 5 | 6 | 0 | 0 | 24 | 0 |
| Defense | 23 | 23 | 0 | 0 | 0 | 0 | 23 | 0 |
| Maritime | 22 | 21 | 1 | 1 | 0 | 0 | 22 | 0 |
| Multidimension | 21 | 21 | 0 | 0 | 0 | 0 | 21 | 0 |
| Editing | 19 | 19 | 0 | 0 | 0 | 0 | 19 | 9 |
| Geocoding | 19 | 17 | 2 | 0 | 0 | 0 | 19 | 19 |
| Server | 19 | 19 | 0 | 0 | 0 | 0 | 19 | 0 |
| Territory Design | 19 | 19 | 0 | 0 | 0 | 0 | 19 | 0 |
| AllSource | 18 | 18 | 0 | 0 | 0 | 0 | 18 | 0 |
| GeoAI | 18 | 18 | 0 | 0 | 0 | 0 | 18 | 18 |
| Space Time Pattern Mining | 18 | 18 | 0 | 0 | 0 | 0 | 18 | 13 |
| Reality Mapping | 14 | 14 | 0 | 0 | 0 | 0 | 14 | 10 |
| Crime Analysis and Safety | 13 | 13 | 0 | 0 | 0 | 0 | 13 | 0 |
| Trace Network | 13 | 13 | 0 | 0 | 0 | 0 | 13 | 0 |
| Bathymetry | 10 | 10 | 0 | 0 | 0 | 0 | 10 | 0 |
| Indoor Positioning | 10 | 10 | 0 | 0 | 0 | 0 | 10 | 0 |
| Standard Feature Analysis | 10 | 10 | 0 | 0 | 0 | 0 | 10 | 0 |
| Public Transit | 8 | 8 | 0 | 0 | 0 | 0 | 8 | 0 |
| Data Reviewer | 7 | 2 | 5 | 0 | 0 | 0 | 7 | 0 |
| Linear Referencing | 7 | 7 | 0 | 0 | 0 | 0 | 7 | 0 |
| Oriented Imagery | 6 | 6 | 0 | 0 | 0 | 0 | 6 | 0 |
| Knowledge Graph | 5 | 5 | 0 | 0 | 0 | 0 | 5 | 0 |
| Ready To Use | 5 | 5 | 0 | 0 | 0 | 0 | 5 | 0 |
| Data Interoperability | 2 | 2 | 0 | 0 | 0 | 0 | 2 | 0 |
| **Total** | **2157** | 2027 | 130 | 33 | 84 | 21 | 2052 | 672 |

Distinct arcpy function names across tool rows: 2033. Concept ("How X works") pages indexed: 433 (concept_pages.csv).
Raster functions: 187 rows; file formats/items: 236 rows (83 raster formats); arcpy API pages: 1049.

## Files
- One CSV per toolbox: toolbox,toolset,tool,arcpy_name,url,description,license,status,difficulty,hard_flag,hard_reason,notes plus priority (P0-P4), exact_repro (Y/M/N, "-" if done) and difficulty_basis. hard_flag: Y = HARD, DOC = documented algorithm but moderate/heavy work, N = not flagged (for P4 domain toolboxes: not assessed).
- raster_functions.csv, file_formats.csv, arcpy_api.csv, concept_pages.csv
- hard_items.md (algorithm summaries and URLs), corrections.md (errors in docs/GAP_ANALYSIS.md and docs/gap_list.csv)
