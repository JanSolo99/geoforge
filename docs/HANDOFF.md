# Current handoff

## Latest state (Claude, 2026-10-29) — read this first

Workspace /mnt/media/ArcGIS_Linux, branch main. Work now follows docs/WORK_QUEUE.md (one tool per
landing, commit and push each; see its rules). This run wrapped up the uncommitted pile into four
landings: 1b3a28e Convert Coordinate Notation inputs (Codex's angular/GARS/GEOREF work + Claude's
UTM/MGRS/USNG input), 0643f03 Field Statistics To Table DATE, 510daa8 Extend Line, and Trim Line
(the commit carrying this handoff). Shared files were staged per landing from the working copy
(/tmp/fstg/stage.py), so each commit holds only its own lines. Validation: full scripts/check EXIT 0
on the whole working tree (210 Rust passed / 5 ignored; /tmp/unigis-trim-check.log); push status
is in the line below.

Still uncommitted on purpose:
- Codex's raster unary integer-float slice (crates/unigis-raster/src/{local,registry,stats}.rs,
  crates/unigis-cli/tests/unary_band_safety.rs, the `umintfloat_*` CASES block in
  scripts/make-arcgis-reference.py, testdata/raster/unary_integer_float/,
  scripts/research/unary_multiband/integer_float/): Codex recorded comparisons as pending.
- Coordinator/queue infrastructure not written by Claude: AGENTS.md, .githooks/post-commit,
  scripts/agents-remote, tools/agent-coordinator/*, scripts/work_queue.py, docs/WORK_QUEUE*.{md,txt},
  docs/dashboard.html. Never read/add NOTES.md or .worktrees; scratch_te/ is scratch.

Next action: next queue item: Assign Weights By Pairwise Comparison (docs/WORK_QUEUE.md #1).

Older notes follow (history; the commit list above supersedes their "uncommitted" remarks).

## Claude run (2026-10-29, after Codex) — read first

Continued Codex's next action 2 (alternative): UTM_BANDS/UTM_ZONES/MGRS/USNG input. All edits
UNCOMMITTED and interleaved with Codex's uncommitted coordinate-notation work in the same files
(grid_notation.rs, construct.rs, registry.rs, coordinate_notation_safety.rs, make-arcgis-reference.py,
manifest.json): commit the whole coordinate-notation slice together, or not at all.
- grid_notation.rs: inverse Krüger TM, inverse UPS, parse_utm/parse_mgrs (+ unit test of 30 probed
  strings). construct.rs: projected inputs dispatched as grid inputs; outputs limited to
  DD_NUMERIC/DD_1/DD_2. registry.rs: input choices + help. Safety test: projected inputs refuse
  DDM_1/DMS_2/GARS/MGRS without touching existing files.
- Fixtures testdata/geoproc/grid_input_{mgrs,usng,utm_bands,utm_zones}.dbf (ArcGIS-made, recipe
  CSVs + make_projected_inputs.py in scripts/research/coordinate_notation/grid/); 8 reference
  cases ccn_grid_input_{mgrs,usng,utm_bands,utm_zones}_{dd_numeric,dd_1} + mgrs_dd_2. Findings:
  grid/projected-inputs.md. ROADMAP and KNOWN_GAPS updated.
- Incident: generating 'ccn_grid_input_*_dd_1' also regenerated Codex's gars_dd_1/georef_dd_1;
  their manifest entries equal Codex's baseline (/tmp/unigis-gars-start-manifest.json, ignoring the
  temp log path) and compare.py passes both.
- Validation: compare.py PASS for all 8 new cases and the 2 regenerated ones; unit tests and
  coordinate_notation_safety PASS. Full scripts/check: first run EXIT101 at clippy
  (type_complexity in the new unit test; fixed with type aliases), rerun EXIT 0: 209 Rust passed,
  5 ignored, Python/fmt/clippy -D warnings PASS (/tmp/unigis-ccn-proj-check2.log). Covers all
  current tree edits (Codex's + this run). Nothing committed or pushed.
- Follow-up (this run, 2026-10-29 later): arcpy projectAs reproduces CCN exactly; 70-sample
  error survey saved (grid/projected_inverse_samples.json, findings appended to
  grid/projected-inputs.md): error ~x^7 on the equator, sign flips with latitude, explodes near
  the pole -> truncated series, formula not identified. Not modelled; guard unchanged.
- Field Statistics To Table DATE (this run, uncommitted): tables.rs FieldStatisticsType
  {Numeric, Text, Date} replaces the numeric flag; DATE columns Minimum/Maximum/Mean/Median/
  Count/NumberofUniqueValues/Mode/LeastCommon/Range(text)/FirstQuartile/ThirdQuartile (dates,
  quartiles interpolated like numbers, Mode/LeastCommon None on ties). Range text: years(365 d)
  >365 d, months(30 d) >30 d, weeks >7 d, days >=1 d, hours >3600 s, minutes >60 s, seconds >=1 s,
  else milliseconds; integer -> no decimals, else 1 decimal if within 1% relative, else 2
  (fits 94/94 probes). SUM etc. in a DATE run -> ERROR 000800 listing the DATE columns.
  registry.rs: field_type choice Date. Fixture testdata/geoproc/dates.dbf (recipe
  scripts/research/field_statistics/make_dates.py); 4 cases fst_dates_date{,_by_grp,_stats},
  fst_err_date_numeric_stat; ogrinfo diff identical; unit test date_ranges_match_arcgis.
  Independent of the coordinate-notation slice except shared files (registry.rs,
  make-arcgis-reference.py, manifest.json, ROADMAP/HANDOFF); tables.rs is only this change.
  Validation: full scripts/check EXIT 0, 210 Rust passed / 5 ignored, Python/fmt/clippy PASS
  (/tmp/unigis-fst-date-check.log; whole tree incl. Codex's and the projected-input edits).
  Not committed (shared files hold Codex's uncommitted work).
- Extend Line (this run, uncommitted): new crates/unigis-tools/src/extend.rs (only this
  change), `pub mod extend` in lib.rs, registry entry/runner `extend_line` (input, length,
  extend_to FEATURE|EXTENSION). Fixture testdata/geoproc/extend_lines.shp (recipe
  scripts/research/extend_line/make_inputs.py), 6 ext_* cases (generator CASES + RUN +
  IN_PLACE entries). Findings: scripts/research/extend_line/README.md. All 6 cases equal
  ArcGIS (per-feature GeoJSON compare, 6 decimals). ROADMAP/KNOWN_GAPS updated.
  Validation: full scripts/check EXIT 0, 210 Rust passed / 5 ignored, Python/fmt/clippy PASS
  (/tmp/unigis-extend-check.log; includes tools_match_arcgis_output over the ext_* cases).
- Trim Line (this run, uncommitted): new crates/unigis-tools/src/trim.rs (only this change),
  `pub mod trim` in lib.rs, registry entry/runner `trim_line` (input, dangle_length,
  delete_shorts DELETE_SHORT|KEEP_SHORT). Fixture testdata/geoproc/trim_lines.shp (recipe
  scripts/research/trim_line/make_inputs.py), 8 trim_* cases (CASES + RUN + IN_PLACE).
  Findings: scripts/research/trim_line/README.md (supersedes the older Trim Line probe notes
  below). All 8 cases equal ArcGIS per feature (GeoJSON, 6 decimals, same feature counts).
  Validation: full scripts/check EXIT 0, 210 Rust passed / 5 ignored, Python/fmt/clippy PASS
  (/tmp/unigis-trim-check.log; includes tools_match_arcgis_output over the trim_* cases).
- Next: Codex's angular GARS item, Aggregate Points, Field Statistics To Table combined
  NUMERIC+TEXT run (needs two outputs).

## Objective / result

Continue recorded next action: resolve GARS re-encoding boundaries. Separated
numeric-input serialization from grid decoding with fresh black-box controls.
Scoped grid-input GARS path now matches all 240 old controls plus 1,144 independent
controls exactly, enabling GARS output for GARS/GEOREF inputs. Legacy angular
path intentionally unchanged; a newly quantified numeric boundary gap remains.
Scoped grid-input GARS slice complete. Final local `scripts/check` EXIT0:
208 Rust passed, 5 ignored; 8 Python passed; fmt and clippy passed. Rare SBN
layout differences remain documented and covered by strict fixture checks.

## Changes this run (uncommitted)

- `crates/unigis-tools/src/grid_notation.rs`: GARS decoding in positive integer
  grid ticks /12, origin subtraction last. New gars_from_grid: degree/radian
  roundtrip then direct global five-minute tick floors, no snapping. One new
  oracle-based boundary unit test. Generic angular encoder remains unchanged.
- `construct.rs`: grid GARS branch and guard enabled; other projected-grid
  outputs still refused. Registry help updated. coordinate_notation_safety.rs
  retains eight unsupported combinations and exact output-byte preservation.
- Four generator/manifest/reference additions: two ordinary GARS output cases
  (81/87 rows) reused from saved research outputs, source attributes/counts
  verified unchanged; no redundant external GP rerun. Two new typed DBF holdout
  cases (652/492 rows), created/probed/generated with ArcGIS.
- `testdata/geoproc/gars_boundary_{gars,georef}.{dbf,cpg,dbf.xml}`; reproducible
  recipe CSVs under research/grid, generated by make_gars_boundary_inputs.py.
- `crates/unigis-shapefile/tests/sbn.rs`: strict exact-entry/layout checks for
  two boundary fixtures; four older exceptions unchanged. Format documentation
  records six ancestor-retention differences; no SBN runtime changes.
- Research scripts/results/models, README, ROADMAP, KNOWN_GAPS updated.
  Durable findings/reproduction: `scripts/research/coordinate_notation/grid/gars-boundaries.md`.

## Evidence / decisions

`probe_gars_numeric.py`: 1,920 numeric DD_2 inputs (240 corners, decoder-arithmetic
variant and adjacent f64 neighbors); source CSV cursor preserves all f64 inputs.
A 15-significant-digit conversion + degree/radian roundtrip + global tick floors
predicts all numeric oracle outputs. Only135/240 numeric centers match saved
GARS->GARS results; 239/240 cells ignore adjacent-ULP changes. This isolates
numeric serialization from decoder math; inferred model, no claim about internals.

For GARS input, origin subtraction after positive grid decoding plus angle
roundtrip/global floors reproduces the saved240 holdout, including 116LR17->
115LR29 and161AA34->161AA37. Fresh seed91762 adds652 GARS and492 GEOREF controls
(480 random each, systematic subcells, short forms, precision2..9, edge/invalids).
Model and native CLI match all1,144 new strings exactly. No coordinate comparison
tolerances changed; SBN layout validation is separately documented below. Files: gars_numeric_results.json, gars_holdout_results.json,
gars_model.py, encoder/decoder candidate scripts and probe_gars_*.py.

`diagnose_gars_numeric.py` also measured1,169 legacy angular-output differences
against those1,920 numeric oracle cases; the existing angular path stays unchanged.
Before integrating that model, probe Double/Single/integer/text fields and source
backends; do not assume all coordinate syntaxes use the same serialization.
This confirmed gap is now explicitly in KNOWN_GAPS.

## Validation

- `cargo build -j3 -p unigis-cli` PASS, /tmp/unigis-gars-build.log. Final error
  wording updated afterward; full gate recompiles that unchanged logic.
- `python3 scripts/research/coordinate_notation/grid/compare_gars_controls.py`:
  1,384 exact strings PASS, /tmp/unigis-gars-native-controls.log.
- `python3 scripts/research/coordinate_notation/compare.py --prefix ccn_grid_input_ --count 22`:
  22/22 PASS,3,080 rows, exact schema/text/geometry presence/warning counts,
  numeric tolerance1e-8deg; /tmp/unigis-gars-grid-compare.log.
- Default comparator12 angular cases PASS, /tmp/unigis-gars-angular-regression.log.
  compare_carry_controls.py all5,634 exact PASS, /tmp/unigis-gars-carry-regression.log.
  Updated compare_outputs.py all326 original GARS/GEOREF fields PASS,
  /tmp/unigis-gars-old-grid-regression.log.
- Diagnostic (not a passing angular compatibility claim): inferred numeric
  model matches1,920; unchanged native angular path differs1,169;
  /tmp/unigis-gars-angular-diagnostic.log.
- First `scripts/check` EXIT101, /tmp/unigis-gars-check.log: full ArcGIS
  comparison passed, then stale refusal list still included GARS (the attempted
  edit missed rustfmt's one-line list). Corrected test to exactly the four
  remaining projected formats, eight source/target combinations. No product fix
  needed; new GARS conversion had correctly succeeded.
- Second `scripts/check` EXIT101, /tmp/unigis-gars-check2.log: reached SBN
  test; 651/657 indexes byte-identical, four existing exceptions plus two new
  boundary fixture differences. Independent parser confirms identical headers,
  record IDs and byte boxes (646/486 indexed records), consistent SBX offsets
  and valid node bounds. Exactly three records per fixture sit one level higher
  in ArcGIS, matching the documented retention gap. Diagnostic:
  grid/inspect_boundary_indexes.py, /tmp/unigis-gars-index-comparison.json.
  Added strict fixture-specific checks of exact six node moves plus headers,
  IDs/boxes, SBX offsets, bin use/counts and tree containment. No runtime SBN
  change. Focused `cargo test -j3 -p unigis-shapefile --test sbn -- --nocapture`
  PASS (/tmp/unigis-gars-sbn-check.log):651 byte-identical,2 checked layouts,
  only the four prior known exceptions. Format docs/ROADMAP/KNOWN_GAPS corrected
  former blanket byte-identity claims.
- Third `scripts/check` EXIT101 at clippy: two new constant chunks_exact uses;
  corrected to as_chunks; /tmp/unigis-gars-check3.log.
- Final `scripts/check` EXIT0, /tmp/unigis-gars-check4.log: 208 Rust passed,
  5 ignored, 8 Python passed, fmt and clippy -D warnings passed. Covers all final
  Rust edits and four reference additions, including ArcGIS comparisons, safety,
  GDAL format checks and shapefile/geodatabase round trips.
- Final Python syntax (31 files) and `git diff --check` PASS. Manifest audit:
  all2,118 starting cases and metadata unchanged, exactly4 additions. Inherited
  hash audit:14 files unchanged; scripts/agents-remote changed externally (below).
  Baselines: /tmp/unigis-gars-start-{hashes,manifest}.json. Testkit/core/CSV/
  raster/coordinator edits preserved. Independent index diagnostic now creates
  temporary native copies itself and reproduces all six moves.

## Inherited state

All earlier angular/GARS/GEOREF/carry parsers, core has_object_id metadata, CSV
synthetic-ID override, ORIG_OID policy, file-oracle leading-space normalization
and OGR_WKT_ROUND=NO remain intact. Prior gate207Rust/5ignored+8Python/fmt/clippy
PASS (/tmp/unigis-carry-check3.log). Relevant findings: grid/carry.md and
`docs/formats/{shapefile-leading-spaces,gdal-wkt-rounding}.md`.

Unfinished raster integer-float work (raster local/registry/stats, unary safety,
umintfloat generator hunk, unary_integer_float fixtures/research) remains
inherited;39 private controls in /tmp/unigis-integer-float-unary; integration/
docs/final comparisons pending. Also inherited: dashboard, agents-remote,
coordinator code/docs/tests and scratch_te/. Never read/add NOTES.md or .worktrees.
Do not stage shared generator wholesale or commit/push unless requested.

## Next actions

1. Grid-input GARS slice complete and validated; edits remain uncommitted. No
   running builds or probes. Keep projected-grid output guards until verified.
2. Next: isolate angular GARS serialization across source numeric types and
   text formats, then apply the inferred model only where new controls prove it.
   Numeric CSV data already explains 1,920 cases; avoid repeating those probes.
   Alternatively begin UTM/MGRS/USNG input inversion.
3. SBN ancestor-retention rule remains unresolved; exact known-layout checks
   preserve all record/box/containment validation while documenting byte gaps.
4. CSV inferred-coordinate schema, custom id_field/name collisions and 64-bit
   ORIG_OID remain known gaps. Read roadmap/gaps before selecting work.
   Hosted Actions exhausted: use local scripts/check, no hosted CI.

Workspace observation: inherited scripts/agents-remote changed externally at
2026-10-28T23:59:23Z after the starting hash capture. This task has not edited
that file; preserve the new content. Other 14 inherited file hashes unchanged.
