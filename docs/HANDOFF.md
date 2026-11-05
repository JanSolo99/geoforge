# Current handoff

Updated 2026-11-05; last agent Codex.
Workspace `/workspace/geoforge`, branch `codex/clean-runtime`, feature commit `8c51cfb2`.
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

## Clean-runtime installation feature

- Agent workflow follow-up is draft PR #2:
  https://github.com/JanSolo99/geoforge/pull/2 (commit 749ea792).
  Readiness PR #1 was merged by the user; main's corpus-fetch improvements are preserved.
- Completed the handoff's clean-target CLI installation item without touching GDB
  reading (Claude is working on that). No Rust/app algorithm changes.
- Added runtime.py to the package: validates binary/database hashes and passing report,
  runs launcher/registry, CSV-to-GeoPackage, analytic Web Mercator and inverse projection.
- clean-runtime.sh verifies archive checksum, extracts, builds a minimal Ubuntu 24.04
  runtime and runs the packaged verifier as non-root, read-only, network disabled.
  No source checkout, GDAL, system PROJ database or compiler is available in the test.
- Integrated mandatory installation check before alpha artifact upload, with failure
  report retention. Pinned alpha runner to ubuntu-24.04. Roadmap item checked off.
- Real container test passed (glibc 2.39), including package/work paths with spaces.
  Evidence: /workspace/readiness/runtime-integrated/ubuntu24.json and image-id.txt;
  /tmp/clean-runtime.log. Tested binary SHA unchanged from the validated alpha.
- Seven package/runtime regression tests passed; actionlint and diff checks passed.
  Full root gate passed: 293 Rust tests, 5 ignored; 30 Python tests; strict Clippy,
  fmt and no-default-features check. Log /tmp/unigis-runtime-gate.log.
  Agent PR #2 also passed GitHub CI run 42015848249.

## Next actions

1. Clean-runtime work is published as draft PR #3:
   https://github.com/JanSolo99/geoforge/pull/3 (feature commit 8c51cfb2).
   Its new GitHub CI run is pending; full local validation passed.
   PR #3 is stacked on #2; retarget to main after #2 merges.
   No automatic merge or deployment. Final clean-source archives and checksum
   files are in /workspace/readiness/dist; installation reports are in
   /workspace/readiness/runtime-final.
2. Keep Claude's GDB-reading work separate. Native desktop/app-workspace checks,
   other browser/OS/architecture coverage and live licensed ArcGIS probes remain.
3. Agent settings on the workstation are untouched: use docs/AGENT_WORKFLOW.md to
   select an available workhorse. No actual provider cost/quality benchmark claimed.
4. Main branch protection remains disabled; repository settings were not changed.

## Running work

Root scripts/check finished successfully; no Cargo build is running. Container tests
finished and their temporary containers were removed. Artifacts are under
/workspace/readiness. This agent has not merged or deployed changes. The original
workstation's edits and coordinator state were not accessed.
