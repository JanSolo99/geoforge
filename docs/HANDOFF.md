# Current handoff

## Latest (Claude, 2026-10-29 evening) — read first

- Codex's Migrate Date Field To High Precision (staged, its full gate EXIT 0 at
  /tmp/unigis-mdhp-check.log, Codex's run hit a usage limit) was committed unchanged from the
  index and pushed as 903bf21.
- Validate Join (this run): crates/unigis-tools/src/validate_join.rs + registry runner/entry
  `validate_join` + `pub mod validate_join`; 5 vj_* cases (TABLE_OUT); ogrinfo diff identical.
  Notes scripts/research/validate_join/README.md; gaps in KNOWN_GAPS.
  The reference test now reads ERROR codes from every line of a message (ArcGIS put
  WARNING 000970 and ERROR 001706 in one message). Validation: full scripts/check EXIT 0,
  245 Rust passed / 5 ignored (/tmp/unigis-vj-check2.log).
- /tmp is a 7.3 GB tmpfs mostly used by /tmp/opencode (not ours); a pre-push gate failed once with
  "No space left on device" in to_geodatabase; stale /tmp/gf-arcgis-ref-*, /tmp/gf-to-gdb-* dirs
  from killed test runs can be removed.
- Validate Join landed and pushed as 001d7a1.
- Adjust 3D Z (this run): crates/unigis-tools/src/adjust_z.rs + registry runner/entry
  `adjust_3d_z`; 6 a3z_* IN_PLACE cases (Z shapefiles in testdata/shp); ogrinfo diff identical
  except one z at 1e-14. Dropped a3z_m_mm (ArcGIS z-domain check, WARNING 002631; in
  KNOWN_GAPS). Validation: full scripts/check EXIT 0, 245 Rust passed / 5 ignored
  (/tmp/unigis-a3z-check.log).
- Adjust 3D Z landed and pushed as d34a600.
- Calculate Geometry Attributes (this run): crates/unigis-tools/src/calc_geometry.rs (maps keywords
  onto add_geometry_attributes, plus hole/curve counts) + registry runner/entry; fixtures
  testdata/geoproc/cga_{parcels,roads,wells}.shp (recipe in
  scripts/research/calculate_geometry_attributes/); 5 cga_* IN_PLACE cases; ogrinfo diff identical.
  Validation: full scripts/check EXIT 0, 245 Rust passed / 5 ignored (/tmp/unigis-cga-check.log).
- Calculate Geometry Attributes landed and pushed as 1b1bfc8.
- Urgent web fix (user request): unigis-tools must compile without default features (web app,
  no PROJ). geodetic_densify (construct.rs), convert_coordinate_notation and table_to_ellipse
  (registry.rs) have `cfg(not(feature = "proj"))` stubs returning "... needs the proj feature"
  (these edits were already in the working tree, uncommitted; reviewed and kept); the
  dm_geometry.rs helpers used only by proj code are gated too (no dead-code warnings).
  scripts/check now runs `cargo check -j3 -p unigis-tools --no-default-features` after clippy.
  app/Cargo.lock carries chrono-tz (from Convert Time Zone). Validation: full scripts/check EXIT 0
  (245 Rust passed / 5 ignored, /tmp/unigis-wasmfix-check.log); `cargo check -p unigis-tools
  --no-default-features --target wasm32-unknown-unknown` clean (no warnings).
- Web fix landed and pushed as cdb7d7d.
- Create Database View DEFERRED (docs/WORK_QUEUE_DEFERRED.txt): works on FileGDB in ArcGIS (a SQL
  view catalog item read as a table), but needs FileGDB view catalog writing plus SQL evaluation
  in the reader.
- Next action: next queue item: Extract Data From Geodatabase (management.ExtractDataFromGeodatabase).
  Parameters probed: in_data (datasets), extract_type DATA|SCHEMA_ONLY, out_type GEODATABASE|XML|
  NEW_FILE_GDB|NEW_MOBILE_GDB, out_folder_path/out_name, get_related_data GET_RELATED (default),
  geometry filter (INTERSECTS|CONTAINS), all_records_for_tables. Plan: scope to NEW_FILE_GDB with
  feature classes/tables, DATA and SCHEMA_ONLY, domains (writer supports them) and relationship
  classes (writer has add_relationship_class); needs a reference-harness comparison of a new
  geodatabase output (see Codex's common/schema_edits.rs for gdb comparisons).

Updated 2026-10-29T09:44:00.347279+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux, main; HEAD and origin/main both
99d534c604c4aea0ac5be4305c66e3c49eafd142. Index empty. User objective: finish/wrap current tool, then build next
WORK_QUEUE tool, with public ArcGIS evidence, full gate and separate landing.
Enable COGOc241a91 was already pushed/complete on startup. This turn implemented
Enable Editor Tracking99d534c (143 scoped paths), observed pushc241a91..99d534c
main->main. Objective completed; no unfinished product edits. Final handoff
checkpoint intentionally uncommitted. No extra agents/coordinators launched;
inherited unrelated edits preserved.

## Current turn: Migrate Date Field To High Precision (uncommitted)

Initial preparation failed because AlterField does not accept DATEHIGHPRECISION;
no fixture saved then. Corrected probe uses public migration tool to prepare the
already-high-precision field; separate /tmp/unigis-migrate-date-probe-fixed root.
12 controls complete (6 successes,6 refusals), results/catalog format states in
scripts/research/migrate_date_precision/. Source migrate_date_base.gdb saved once;
shapefile fixture copied from disposable public control. Date precision0->1 and
HighPrecision=true metadata only; aliases/values/NULL retained. Table/field header
versions unchanged3/4. Already-high field retains123ms. Mixed valid/wrong types
can PARTIALLY migrate in ArcGIS; native preflights atomically, documented gap.

Native writer/date_precision.rs validates Date types/names/duplicates then splices
HighPrecision markers within one journal, no user table rewrite. Registry/context/
CLI FileGDB host added; other hosts explicitly fail. Date fields use semicolon
lists, case-insensitive canonical matching. Two new safety tests protect user
bytes, no-op/repeat identity and atomic refusals. Generic schema comparator checks
precision flags.12 mdhp_* references generated PASS /tmp/unigis-mdhp-references.log.
Targeted cargo test -j3 -p unigis-cli --test migrate_date_precision_safety --test
arcgis_reference PASS4 /tmp/unigis-mdhp-tests.log (no warnings). Independent public
ArcGIS reader PASS6 successes exact precision/schema/values/NULL/geometry,
/tmp/unigis-migrate-date-native.{log,json}, saved native_reader_results.json.
Formatting/Python syntax/whitespace, manifest2295 unchanged+12 and inherited hash
preservation audits PASS. Docs ROADMAP/KNOWN_GAPS/formats updated. Full scripts/check
running /tmp/unigis-mdhp-check.{log,exit,pid}, PID1539872; Python8/fmt/clippy
-D warnings PASS; full reference suite and both migration safety tests PASS,
remaining workspace format/engine tests running; no overlapping Cargo/Trunk.
123 scoped paths staged, source/staged whitespace and preservation audits PASS.
No commit until exit0. No extra agents/coordinators or external web publish.

## Completed implementation

- filegdb/writer/editor_tracking.rs enable_editor_tracking configures simple class/
  table roles in one existing journal. Supports optional roles, ADD_FIELDS and
  default NO_ADD_FIELDS, UTC/default and DATABASE_TIME. Existing roles/time basis
  are retained on attempted switches; additional roles can be enabled.
- Missing user/date fields become nullable text255/low-precision Date with NULL
  values. Existing aliases/types/lengths/values/OIDs/geometry remain. All names and
  types preflighted before changes. Missing fields require one rewrite using
  normal spatial-grid recalculation, with unrelated binary flags restored.
  Otherwise only catalog/descriptor flags change. Repeat/active no-ops retain bytes.
- Roles/EditorTrackingEnabled/IsTimeInUTC and GPFieldInfoEx Editable=false set with
  physical editable mask0x04 cleared. Missing root properties use the actual XML
  closing tag, including native prefixed roots. Other properties/domains retained.
- Registry editor_tracking::ENABLE, optional Context host capability, CLI FileGDB
  host. Unsupported hosts fail explicitly. No roles001218, missing001278, wrong
  type001217 and duplicate roles001214 match primary public errors. Secondary/
  repeated ArcGIS diagnostics omitted. Conventional attachment tables refused.
-14 etrack_* refs:10 successes/4 refusals. All2281 prior cases unchanged;2295 total.
  Existing schema comparator covers roles/time basis, XML AND physical readonly
  flags, fields/values/OIDs/geometry/CRS/domains/index definitions and repeat.
 3 new safety tests protect data/NULL additions/unrelated files, readonly flags,
  no-op/refusal byte identity and newly created native table root metadata.
- Public recipes/results/raw catalog and independent reader evidence in
  scripts/research/enable_editor_tracking/. Source fixture
  testdata/geoproc/enable_tracking_base.gdb saved once before disposable controls.
  Do not recreate merely for context. GDAL read saved file formats only;
  public ArcGIS Pro3.7 is the GP oracle. No SDK/runtime GDAL dependency.
- ROADMAP/KNOWN_GAPS updated. Scope: simple FileGDB classes/tables. Feature datasets,
  relationships/attachments/advanced class extensions, enterprise/mobile/layers
  unsupported/unverified. Conventional attachment names refused; renamed schemas
  unverified. High-precision date roles refused. Unusual casing/defaults/domains/
  ZMcurves and rare invalid combinations unverified. Native edit APIs do NOT
  automatically stamp users/timestamps on later edits; metadata configuration is
  verified but that broader edit engine behavior remains a documented gap.

## Validation and decisions

- ug tools confirmed absent before implementing; official public reference read.
 14 public controls confirm existing data not backfilled/converted, role additions,
  readonly flags, defaults, existing state retention and primary refusal codes.
- uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'etrack_*' PASS;
  /tmp/unigis-etrack-references.log,10 successes/4 expected failures.
- cargo test -j3 -p unigis-cli --test enable_tracking_safety --test arcgis_reference
  PASS4 /tmp/unigis-etrack-tests.log. Afterwards added native prefixed-root handling
  and third safety test; the final full gate covers those additions.
- Independent public arcpy read PASS for ALL10 successes, exact roles/time basis,
  field schema/aliases/length/nullability/required/editable flags, values/geometry.
  /tmp/unigis-enable-tracking-native.{log,json}, native_reader_results.json;
  disposable /tmp/unigis-enable-tracking-native.gdb.
- Extra public reader probe_native_root.py PASS on final CLI native-created table
  with newly inserted root properties; all roles/UTC recognized and fields readonly.
  /tmp/unigis-enable-tracking-native-root.{log,json}, native_root_results.json;
  disposable /tmp/unigis-enable-tracking-native-root.gdb/native. Native Create Table
  was invoked once; no external publish. Initial classic create-table --help lookup
  was unrecognized; registry ug run create_table used correctly.
- Final scripts/check EXIT0 /tmp/unigis-etrack-check.{log,exit,pid}:242 Rust passed,
  5 ignored; Python8/fmt/clippy -D warnings PASS. Covers final root handling, all3
  new safety tests/refs/host capability and full format/engine suites. No product
  edits after gate; research/docs only (extra root reader independently validated).
- Python syntax, staged/source whitespace,143-path scope, old-manifest and inherited
  preservation audits PASS. Filtered /tmp/unigis-etrack-stage.patch excludes the
  inherited4-line umintfloat generator hunk. No Cargo/Trunk/probes running.

## Next action

Next queue item: Migrate Date Field To High Precision
(management.MigrateDateFieldToHighPrecision). Queue regenerated after landing:
1612 missing. Read roadmap/gaps/public reference, run ug tools and public controls.
Do not polish landed tools unless requested. No Cargo/Trunk/ArcGIS probes running.
Enable COGO was already complete on startup and was not repeated.

## Inherited unrelated work

Previous RAM task complete/uncommitted: scripts/publish-web-dist.sh forces1 job,
9GiB admission guard/exclusive lock; scripts/web-build-memory.py, docs/WEB.md and
service/timer files remain outside GP commits. No manual web build/publish or
measured peak this turn. Prior /tmp/unigis-web-memory-validation.log. Timer waits
for idle coordinator/Cargo. Preserve raster local/registry/stats, unary safety/
fixtures/research, generator umintfloat hunk, AGENTS/hooks/agents-remote/coordinator,
queue infrastructure and generated dashboard/WORK_QUEUE. Baseline
/tmp/unigis-dm-inherited-hashes.json (prior WEB edits intentional). Never read/add
NOTES.md or .worktrees; scratch_te scratch. Max Cargo-j3; publisher1; no extra agents.
