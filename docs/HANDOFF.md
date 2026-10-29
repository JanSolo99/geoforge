# Current handoff

## Latest (Claude, 2026-10-30): finishing Codex's Feature Class To Geodatabase — read first

Codex's run hit a usage limit with the tool implemented and 13 fctg_* references generated, but
`tools_match_arcgis_output` failing on "CRS differs" (5 shapefile-import cases). Fix (Claude):
crates/unigis-cli/src/feature_class_to_geodatabase.rs now stores ArcGIS's regenerated WKT
(`Crs::arcgis_wkt()`), as write_output does for other geodatabase outputs. KNOWN_GAPS notes the
catalog still lacks <WKID>/<LatestWKID>. `cargo test -j3 -p unigis-cli --test
feature_class_to_geodatabase --test arcgis_reference` PASS (/tmp/unigis-fctg-tests2.log); Codex's
native ArcGIS reader check: 10/10 matched (/tmp/unigis-fctg-native-reader.log).
Validation: full scripts/check EXIT 0, 275 Rust passed / 5 ignored (/tmp/unigis-fctg-fullcheck.log). Commit scope: Codex's FCTG files (see below), generator
without the umintfloat block; exclude raster/coordinator/hook/AGENTS/web files and the untracked
scripts/publish-web-dist.sh, scripts/web-build-memory.py (not part of this tool).
Landed and pushed as 342fdd7.
Feature Class To Shapefile (this run): crates/unigis-tools/src/feature_class_to_shapefile.rs +
Context::export_shapefiles (registry) + CLI host fn export_shapefiles (main.rs, converts every
input with feature_class_to_feature_class before writing); 3 fc2shp_* WORKSPACE_OUT cases. The
generator now passes list arguments as lists (the bridge translates list elements) and
--annotate skips cases without args (Codex's gids_* cases made it crash). ogrinfo diff identical.
Validation: full scripts/check EXIT 0, 275 Rust passed / 5 ignored (/tmp/unigis-fc2shp-check.log).
Landed and pushed as 3fc9fae.
- Features To GPX DEFERRED (docs/WORK_QUEUE_DEFERRED.txt): ArcGIS's script repeats track points by
  an unidentified rule (x5 / x2), merges multipart lines and fails on non-cp1252 text; byte-exact
  GPX needs those quirks.
- Next action: next queue item: GPX To Features (conversion.GPXtoFeatures). Probed, not
  implemented: findings and probe file in scripts/research/gpx_to_features/ (order tracks, routes,
  waypoints; Id/Name/Descript/Type/Comment/Symbol/DateTimeS/Elevation for a shapefile; PointZM with
  M NaN; TRACKS_AS_LINES one PolylineZM per trk). roxmltree is already a workspace dependency.

Updated 2026-10-29T18:49:14.455806+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD b20d66d2fcbf56bbae05b0591da78ad853e6b7c8.
User objective: wrap completed Excel then build next queue tool, full gate/scoped
commit/push under queue rules. Excel already landed/pushed, no unfinished product
edits inherited. Current tool Feature Class To Geodatabase.

Startup guidance/handoff/status/diffs/ug tools reconciled. No other agents/builds.
Inherited raster/coordinator/hooks/AGENTS/web/queue edits unchanged; protected hashes
`/tmp/unigis-fctg-preserved.json`. Generator inherited umintfloat4-line hunk must
remain unstaged. Do not read/add NOTES.md or .worktrees; no manual web publishing.

Public docs read.10 controls completed in research/feature_class_to_geodatabase/
results.json, `/tmp/unigis-fctg-probe.log`:7 successes/3 expected000732 refusals.
Single/batch/repeat, collision suffix_1/_2, empty, root/feature-dataset destination;
table/missing-input/missing-workspace fail. Polygon/line outputs add required,
noneditable Shape_Length/Shape_Area fields; geometry field case Shape, OIDs reset.
3 additional schema/default/name controls running exec (see /tmp/unigis-fctg-options.log),
script probe_options.py; do not repeat before checking saved options_results.json.
All writes disposable /tmp/unigis-fctg-{probe,options}, source fixtures unchanged.

Implementation now written (not yet validated): registry Report in
crates/unigis-tools/src/feature_class_to_geodatabase.rs and optional Context import
capability; CLI batch importer module + ordinary Context bridge. Entire input batch
loaded before table writes; one existing-FileGDB transaction covers all new classes.
Unique ASCII names and root/feature-dataset outputs, OID reset, aliases/defaults
kept and domains/indexes omitted as public defaults. Opt-in writer standard mode
uses Shape, required/noneditable calculated measure fields; existing mode unchanged.
New TableBuilder flag requires false initialization in extract.rs (initial cargo
check exposed missing initializer; fixed). Empty target feature datasets are resolved
through writer.feature_dataset_srs, since reader enumeration omits empty datasets.

Public options3 controls succeeded: aliases/defaults retained, domains/indexes
omitted, gapped OIDs reset1..n; 9 bad-name -> T9_bad_name. options_results.json.
One-time fixtures fctg_empty.gdb/fctg_seed.gdb/fctg_names created via make_fixture.py;
source fixtures must not be regenerated. `/tmp/unigis-fctg-fixtures.log`.
13 fctg_* full-workspace reference cases generated (10success/3expected000732).
Generic workspace_reference manifest helper supports saved seed + registry tool
execution and compares complete catalog/fields/rows/geometry/CRS. Reference script
adds own CASES/RUN/function/dispatch hunks atop inherited umintfloat4-line hunk.

`cargo test -j3 -p unigis-cli --test feature_class_to_geodatabase --test arcgis_reference`
RUNNING exec12908, `/tmp/unigis-fctg-tests.log`. Includes invalid-batch/late projection
rollback and same-workspace repeat/source preservation tests. No other builds.
No final validation yet; initial cargo check failed only missing TableBuilder flag.
Next: resolve targeted tests, native ArcGIS reader, scope/gaps/roadmap docs; full
scripts/check -j3 sequential; scoped commit/push then queue refresh. No commit/push.
