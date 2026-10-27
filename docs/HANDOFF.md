# Current handoff

Updated: 2026-10-27T14:02:55+00:00; last agent: Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`; HEAD `c3abd4a` (field support committed; push pending).
Remote SHA verified cdc23aac785765e07bfc3cb053bf44ac5900eade. User authorizes ongoing
commits/pushes. Do not launch another agent, CLI session or coordinator.

## Objective

User requests sustained development, validated commits, pushes and dashboard updates
without repeated “continue” prompts, until unable. Recovered prior Claude partial
Graphic Buffer work from `.git/agent-coordinator/20261027T101238-693a3583/`; latest request
takes precedence over old chat. Basemaps, branding and dock work are already landed.

## Completed and pushed this Codex run

- d66383c: finish inherited Graphic Buffer using native offsets, caps/joins and hole
  ownership; all ten original reference cases pass, without tolerance changes.
- ebc2d2d: dashboard counts actual tool headers and registered external modules,
  excludes parameter names and reference-only unimplemented tools. Four offline tests
  added to scripts/check and GitHub CI. Both early GitHub CI runs SUCCESS:
  https://github.com/JanSolo99/geoforge/actions/runs/40512024122
  https://github.com/JanSolo99/geoforge/actions/runs/40512020282
- cdc23aa: 18 further Graphic Buffer cases, negative/zero polygon distances, numeric
  reserved-field replacement and miter limits 1/1.25/2/10 including acute corners.
  ArcGIS bevels over-limit miters; native clipping was incorrect. Unioned overlapping
  tip patches on a bevel base match all 28 cases. Full root gate and mandatory
  pre-push repeat passed: 174 Rust tests, 5 ignored, 78 result groups, four Python
  dashboard tests, fmt, clippy -D warnings. Push exit 0, remote verified.
  Logs: /tmp/unigis-gb-expanded-check.log, /tmp/unigis-gb-expanded-push.log.

## Current slice: field support committed as c3abd4a; dashboard follow-up ready

- Source crates/unigis-tools/src/graphic_buffer.rs: case-insensitive numeric/text
  distance fields, numeric null/field-zero skipping, negative point errors, unknown
  field 000728, imperial units using ArcGIS's observed definitions. Unit changes are
  local to Graphic Buffer; do not assume other tools use the same parser.
- Geographic inputs are now refused, so raw numeric distances cannot become degrees.
  Null/malformed text fields refused before any output: ArcGIS sometimes reuses prior
  feature geometry while writing BUFF_DIST=0; we do not reproduce that unsafe quirk.
- Three CLI tests in crates/unigis-cli/tests/graphic_buffer.rs exercise these refusals
  and absence of partial output. All three passed in the completed final gate.
- 20 gb_field_* cases generated (15 successes, 5 expected errors). New synthetic
  inputs graphic_buffer_fields and graphic_buffer_poly_fields, generator entries,
  manifest and output shapefile sets belong to this slice.
- Reproducer/evidence: scripts/research/graphic_buffer/{make_fields.py,
  field_observations.json,README.md}. Public docs and black-box ArcGIS only; GDAL
  used solely for creating/inspecting input files.
- Tiny positive buffers near ArcGIS XY tolerance may collapse there and survive here;
  recorded in KNOWN_GAPS, including the 1 Millimeters observation. No threshold guessed.
- docs/ROADMAP.md and KNOWN_GAPS.md updated; dashboard not yet refreshed for this slice.

## Validation / running processes

- `cargo test -j3 -p unigis-cli --test arcgis_reference`: PASSED all manifest cases,
  including all 48 Graphic Buffer cases; /tmp/unigis-gb-fields-reference.log.
- First gate `/tmp/unigis-gb-field-check.log` stopped at one overly specific error
  assertion: DBF text nulls read as empty strings; refusal and no-output behavior were
  correct. Assertion corrected. Also tightened unknown-unit fallback so recognized
  units never bypass a missing CRS conversion; third safety test added.
- Final `scripts/check` PASSED exit 0, `/tmp/unigis-gb-field-check-final.log`,
  on final Rust source/fixtures and three safety tests: 177 passed, 0 failed,
  5 ignored, 79 result groups; fmt and clippy -D warnings passed. Four dashboard
  tests ran at gate start; eight passed separately after the dashboard follow-up.
  No running builds. Field slice committed separately from dashboard changes.
- Dashboard follow-up (not staged with the field slice): build.py and test_build.py
  preserve fetched CI results in docs/dashboard/ci_runs.json through offline hooks or
  fetch failures, show last-fetched time, and use the latest KNOWN_GAPS section.
  Eight Python tests pass; generated HTML JSON invariants and node --check pass.
- ArcGIS generator and all exploratory probe workers finished. No other agent running.
- cdc23aa GitHub CI in progress: https://github.com/JanSolo99/geoforge/actions/runs/40515889873

## Next actions

1. Field support committed c3abd4a. Commit dashboard follow-up separately (8 Python
   tests plus generated JSON/JavaScript checks passed); snapshot refreshed with --ci.
2. Push both normally; mandatory root pre-push gate must pass. Verify remote HEAD/CI.
3. Dashboard cache fix is implemented and tested; stage build/test, ci_runs.json,
   dashboard.html and updated handoff in its own commit.
4. Continue another bounded oracle-backed registry tool (inspect actual ug tools first),
   keeping tested commit/push checkpoints and dashboard current.

## Durable context / preserved unrelated work

Detailed Graphic Buffer findings: scripts/research/graphic_buffer/README.md.
Prior tool-batch findings: scripts/research/sa_batch/README.md. New tools use registry
+ generator RUN + saved ArcGIS cases, not giant clap/test branches. scripts/check is
the mandatory pre-push gate (core.hooksPath=.githooks); do not bypass. Cargo jobs <=3,
no overlapping builds, no local full app CI for root-only edits. Capture ug tools to a
file rather than piping to head (known broken-pipe panic observed; not fixed).

Preserve/exclude unrelated tracked aux.xml changes in testdata/{geoproc,raster},
deleted tracked *.sr.lock files, .worktrees/, NOTES.md, scratch_cf.tif and
scratch_trend_rms.txt.xml. Do not read/use unrelated NOTES.md (proprietary reverse-
engineering material). No reset, stash, worktree transfer or external messaging.
