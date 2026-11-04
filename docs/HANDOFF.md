# Current handoff

Updated 2026-11-04; last agent Codex.
Workspace `/workspace/geoforge`, branch `codex/readiness-alpha`, base HEAD `4aa5e99`.
This is a separate cloud checkout, not `/mnt/media/ArcGIS_Linux`. It began clean.
No local edits from the original workstation were accessed or transferred.

## Objective

User requested the readiness milestone: fix CI, automate representative workflows,
report interface-specific coverage, and package a tested CLI alpha. Work is scoped
to readiness, not the next geoprocessing queue item.

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
  Artifact: `/workspace/readiness/dist/unigis-cli-alpha-4aa5e99d8c0d-dirty.tar.gz`.
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

## Next actions

1. Review and prepare a draft PR for the validated changes. No merge, GitHub Release
   publication or deployment is authorized as part of this milestone.
2. Require CI through branch protection if desired (main is currently unprotected).
3. Run the native desktop/app-workspace gates and a clean-target installation test
   before broadening release claims; see READINESS.md.

## Running work

No Cargo build or test is running. Reports and artifacts are under `/workspace/readiness`.
The original workstation's unrelated edits and coordinator state were not accessed.
