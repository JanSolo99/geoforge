# Current handoff

Updated 2026-11-05; last agent Codex.
Workspace `/workspace/geoforge`, branch `codex/agent-productivity`, based on main `850eb8fa`.
This is a separate cloud checkout, not `/mnt/media/ArcGIS_Linux`. It began clean.
No local edits from the original workstation were accessed or transferred.

## Objective

User requested the readiness milestone: fix CI, automate representative workflows,
report interface-specific coverage, and package a tested CLI alpha. Work is scoped
to readiness and cheaper-agent productivity, not the next geoprocessing queue item.
Latest user request: improve agent instructions and build skills to reduce reliance
on premium models.

## Completed and validated

- Reproduced the FileGDB oracle failure with GDAL 3.10.3. Upstream GDAL fixed the
  non-editable-default field reader in 3.14 development (#15385). The specific
  fixture now has direct native field/value assertions; only its older-GDAL
  comparison is gated. Corpus download pinned; cache replacement follows extraction.
- Full root workspace tests: 293 passed, 5 ignored, zero failures. Saved ArcGIS
  comparator and exhaustive copy-to-geodatabase test ran. Log `/tmp/unigis-tests.log`.
- Strict all-target Clippy, fmt check, and unigis-tools without default features pass.
  Logs `/tmp/unigis-clippy.log`, `/tmp/unigis-fmt.log`, `/tmp/unigis-web-engine.log`.
- Three CLI workflows pass (vector clip/export/reopen, FileGDB edit/export/reopen,
  raster CRS/algebra/NoData/failed-write safety). `/workspace/readiness/cli.json`.
- Existing web-dist artifact identifies itself as build of `4aa5e99`. Chromium test
  passed import, UI Buffer, export and project save/reload/reopen; no page errors.
  `/workspace/readiness/browser.json` and `browser.png`. External requests were
  deliberately blocked. Native desktop and app-workspace checks were not run.
- Dashboard macro registration and official-name mapping fixes: dashboard and queue
  now agree on 474/2028 API entries (406 with references, 68 without); 373 native
  tool names. Missing: 1527 eligible + 27 deferred. These are not parity percentages.
- Dashboard regression tests (10) and package safety tests (3) pass.
- Optimized CLI build passed; all three workflows passed for it and for the extracted
  alpha launcher under a directory containing spaces. The package includes PROJ data
  and its copyright notice; binary and database hashes checked after extraction.
  Reports: `/workspace/readiness/cli-release.json`, `cli-relocated.json`.
  Artifact: `/workspace/readiness/dist/unigis-cli-alpha-abcb24c75884.tar.gz`.
- actionlint 1.7.7 and shell/Python/JavaScript syntax checks pass.
- Added PR CI, reusable engine verification before web publication, a browser gate,
  and a manual tested-CLI artifact workflow. Added ALPHA.md and READINESS.md.

## Environment and reproducibility

Rust 1.99.1 installed under /workspace; commands source `/workspace/unigis-build-env.sh`.
This uses the exact pinned compiler with debug info/incremental compilation disabled
for available disk space. Native CMake/SQLite dependencies extracted under
`/workspace/build-deps`; no system installation was possible or required.
GDAL is a format oracle only. ArcGIS/its bridge is unavailable here; no new ArcGIS
reference outputs were generated. Browser tests use the existing WASM, not a rebuild.

## Agent productivity follow-up

- Added canonical project skills for delivery, CI triage and ArcGIS tool development,
  with Claude discovery wrappers. AGENTS.md routes tasks and bounds context reads.
- Added docs/AGENT_WORKFLOW.md with model setup, task brief and evidence-based
  escalation. Optional ambition review no longer requests automatic periodic use.
- Coordinator supports optional Claude/Codex model environment settings and reports
  requested models in /status; existing CLI defaults and permissions are preserved.
- All 13 offline coordinator tests pass, including new/resumed/native model overrides.
  scripts/check now includes these tests. No paid model calls were used to test this.
- Draft PR: https://github.com/JanSolo99/geoforge/pull/1. Readiness commit abcb24c
  passed GitHub CI run 41920743046. Agent follow-up full local gate passed:
  /tmp/unigis-final-gate.log (293 Rust tests, 5 ignored; 26 Python tests; fmt,
  strict Clippy and no-default-feature check). No Rust/app source changes.

## Next actions

1. Open a separate agent-productivity PR and check CI; readiness PR #1 was merged\n   by the user while this follow-up was in progress. Main's corpus-fetch fixes\n   are preserved.
   Latest user direction: continue completing roadmap/handoff features while they
   sleep; Claude is working on GDB reading. Avoid that area and stay on this branch.
2. Require CI through branch protection if desired (main is currently unprotected).
3. Run the native desktop/app-workspace gates and a clean-target installation test
   before broadening release claims; see READINESS.md.
4. On the workstation, select an available workhorse model using AGENT_WORKFLOW.md;
   actual provider cost/quality has not been benchmarked and settings there are untouched.

## Running work

No Cargo build is running. Reports and artifacts
are under /workspace/readiness. This agent performed no merge, release publication or deployment.
The original workstation's unrelated edits and coordinator state were not accessed.
