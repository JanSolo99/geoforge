# Current handoff

Updated 2026-10-29T08:28:30.163364+00:00; Codex.
Workspace /mnt/media/ArcGIS_Linux, main, HEAD/origin/main7ab7822.
User: finish current tool then build next WORK_QUEUE item, separate landings.
No other agents/coordinators; preserve all unrelated work.

## Completed landing / current tool

Delete Multiple committed/pushed7ab7822 (observed8194f63..7ab7822 main->main).
109 scoped paths; source unchanged since full scripts/check EXIT0 confirmed
/tmp/unigis-dml-check.{log,exit}:232 Rust/5 ignored,Python8/fmt/clippy PASS.
Empty abandoned .git/index.lock from interrupted17:40 staging removed only after
no Git process/file holder was found. Index empty after landing. Generator
excluded inherited umintfloat hunk; web/raster/coordinator changes preserved.

Queue regenerated1616 missing, next Disable COGO (management.DisableCOGO).
ug tools confirmed absent; official docs/probes completed (8 controls).
ArcGIS retains fields/values but clears their Required markers, disabling COGO;
plain/custom lines succeed unchanged, point/polygon error003917, table000732.
Shapefile succeeds unchanged with002051 warnings; native no-op does not emit these.
Public arcpy GDB_Items SearchCursor exposed only root row, so initial Definition
capture was None; public file-format read with GDAL (not GP behavior oracle)
identified removal of5 Required elements in GPFieldInfoEx. Raw evidence saved in
scripts/research/disable_cogo/catalog_format_results.json and probe_results.json.
Fixture cogo_base.gdb saved once from untouched enabled state; reuse it.

Disable COGO implemented (not landed):
- filegdb/writer/cogo.rs transactional metadata splice removes only Required
  elements for Direction/Distance/Radius/ArcLength/Radius2; no user table rewrite.
- tools/cogo.rs runner checks geometry and invokes optional Context capability;
  read-only hosts explicitly fail; CLI implements FileGDB and shapefile no-op.
-8 dcogo_* references generated, /tmp/unigis-dcogo-references.log:5 successes,
  3 refusals; all2,249 prior manifest cases unchanged. Generic schema snapshots
  include cogo required flags; comparator now checks raw catalog Required markers
  (roxmltree dev dependency) as well as retained rows/fields/geometry/indexes.
- Safety test guards user-data byte identity, repeat byte-idempotence and invalid
  geometry; host default explicit-failure test. Native build running/finished
  /tmp/unigis-dcogo-build.log (session40793); inspect before starting another Cargo.
Native build PASS /tmp/unigis-dcogo-build.log. Independent ArcGIS read PASS:
Describe.isCOGOEnabled false, all5 fields deletable, original dimension90/10
and source KEY7/geometry retained; /tmp/unigis-disable-cogo-native.log and saved
native_reader_results.json. No new source fixture/probe needed.
Initial reference test stopped at shapefile case because Report tools mutate
input instead of creating output. Added generic report_in_place fixture handling;
oracle and native now both use byte copies (no CopyFeatures schema conversion),
so originals remain protected. Regenerated only dcogo_shapefile, no old cases
changed. No tolerance changes/skips.
Safety test PASS /tmp/unigis-dcogo-safety.log: user data bytes unchanged,
byte-idempotent repeat and invalid point refusal. Python syntax/whitespace PASS.
Full scripts/check EXIT0 /tmp/unigis-dcogo-check.{log,exit}:234 Rust passed,
5 ignored; Python8/fmt/clippy -D warnings PASS. Covers final native error mapping,
all8 COGO references, raw Required marker comparison, shapefile report adapter,
metadata safety/readonly host, prior tools and format/index/roundtrip tests.
No product changes after gate; only docs. Scope/source/staged whitespace and
inherited preservation audits PASS;120 COGO-only paths staged. Cargo.lock change
is the roxmltree dev dependency for catalog assertions. Generator patch excludes
inherited umintfloat hunk. No Cargo/Trunk/probes running.
Next: commit/push staged COGO implementation separately; regenerate queue and
final checkpoint. Actual next queue item Disable Editor Tracking.
No ArcGIS probe running after reference generation completed. No Trunk started.

## Unrelated work preserved

Previous RAM request complete: publisher one Cargo job,9 GiB MemAvailable/cgroup
check, exclusive lock, no manual publish. scripts/publish-web-dist.sh,
scripts/web-build-memory.py, docs/WEB.md and service/timer files stay outside
GP commits. Read-only tests /tmp/unigis-web-memory-validation.log; no build peak
measured. Timer remains installed; it waits for coordinator/Cargo idle.
Preserve inherited unary raster local/registry/stats,safety,generator umintfloat
hunk/research/fixtures and coordinator/queue infra. Baseline
/tmp/unigis-dm-inherited-hashes.json (WEB changes intentional from later task).
Never read/add NOTES.md or .worktrees; scratch_te scratch. No extra agents.
