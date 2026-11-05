# Readiness milestone

This milestone establishes repeatable workflow checks, rather than equating a
registered tool or a saved reference case with production readiness.

## Cloud validation, 2026-11-04

Base source: `4aa5e99`, plus the readiness changes on `codex/readiness-alpha`.
Rust 1.99.1, Debian 13, GDAL 3.10.3, Chromium 155.0.8059.79.

| Check | Result |
|---|---|
| Root `cargo test --locked -j3 --workspace` | 293 passed, 5 ignored, zero failed; includes saved ArcGIS references and the large GDAL corpus |
| Strict all-target Clippy and rustfmt | Passed |
| `unigis-tools --no-default-features` | Passed (engine configuration used by the browser) |
| Dashboard/package Python regression tests | 13 passed |
| Three CLI workflow checks, development and optimized release binaries | Passed |
| Extracted alpha launcher, relocated under a path containing spaces | All three workflows passed; binary and PROJ database hashes verified |
| Browser import → Buffer → GeoJSON export → project save/reload/reopen | Passed, zero page errors |
| Workflow definitions | actionlint 1.7.7 passed |

The browser test used the existing `web-dist` build of `4aa5e99`, WASM SHA-256
`a376ff5ffcde4a8eb2ba738cff4fcce955bed47ac0cca5c1aa1c9d5ab3e26db4`.
There was no app-source change or fresh WASM build in this milestone. Online
assets were deliberately blocked. These are local cloud validation results;
the cached GitHub CI card remains the separately fetched status of main.

## Capability matrix

| Workflow | CLI | Browser | Native desktop |
|---|---|---|---|
| Clip points, export GeoPackage, reopen Shapefile; preserve CRS/attributes | Automated in `smoke.py` | Not covered by this milestone | Not run here |
| Create/edit FileGDB, calculate field, delete rows, export/reopen | Automated in `smoke.py` | Reading only: a `.gdb` folder (picked or dropped) or a zip holding one is listed in the Catalog under Databases and its feature classes added; `browser.cjs` uploads `testdata/gdb-reference/edit_fc.gdb` as a zip. Folder pick/drop itself is not automated. Editing is desktop-only | Not run here |
| Raster import, projection, algebra, NoData, failed-write preservation | Automated in `smoke.py` | Not covered by this milestone | Not run here |
| Import GeoJSON, Buffer two points, export GeoJSON, save/reopen project | Not this test's scope | Automated in `browser.cjs` | Not run here |
| Basemaps, plugins and online services | Not this test's scope | Explicitly excluded from offline smoke | Not run here |

JSON reports record pass/fail and SHA-256 of the tested binary/WASM. A passing
browser check applies to that build and Chromium version, not all browsers or
every tool. The canvas UI test uses a fixed 1400×900 viewport and checks resulting
data/history/downloads; UI layout changes may require updating its click points.
It uses the existing embedding protocol for UI actions, not a privileged test API.

## Gates and artifacts

- `scripts/check`: dashboard regressions, format, strict Clippy, no-default-features
  compilation and full root workspace tests, with three Cargo jobs and locked dependencies.
- CI runs on pull requests, main pushes and manual requests. It retains gate logs
  and the CLI workflow report. Repository branch protection must require the check
  if merges should be blocked; this milestone does not change repository settings.
- Web publishing requires the reusable engine CI job and the browser smoke check
  on the newly built WASM before either publishing step. Reports/screenshots are
  uploaded even when the browser gate fails.
- The manual alpha workflow builds and smoke-tests the release CLI, then packages
  the exact tested binary. See [ALPHA.md](ALPHA.md).

Run the browser check against a built distribution:

```sh
npm ci --prefix scripts/readiness
(cd scripts/readiness && npx playwright install chromium)
node scripts/readiness/browser.cjs --dist app/dist/web --report /tmp/browser-readiness.json --screenshot /tmp/browser-readiness.png
```

Use `--chromium /path/to/chromium` for a system browser. The script serves the
distribution on a temporary loopback port. External requests are deliberately
blocked and recorded, so tile/plugin availability cannot make this offline test
flaky. It does not test service-worker installation, TLS or live hosting.

## CI failure resolved by this milestone

At main `4aa5e99`, `filegdb_matches_gdal` failed on
`openfilegdb_nonEditable_default.gdb/test`. This was reproduced with GDAL 3.10.3:
GDAL reported a `filegdbtable.cpp` error and returned empty fields/features.
UniGIS read the two fields and three rows expected by GDAL's own regression test.
Upstream fixed the reader in [OSGeo/gdal#15385](https://github.com/OSGeo/gdal/pull/15385),
commit `2c7893c6203ce492a21aafe9ee7a2236b8af024a`, during 3.14 development.

The test now always checks native fields/values for this specific fixture when
running the corpus comparison, and avoids that one GDAL comparison before 3.14.
All other table comparisons remain enabled. This does not relax geometry or value
tolerances. The corpus download is pinned to a reviewed revision, and successful
download/extraction precedes replacement of the old cache.

## Progress accounting

The dashboard now detects registered schema tools declared with `tool!`, including
Delete Mosaic Dataset. The queue separately reports eligible and deferred missing
tools. Neither registered coverage nor reference coverage establishes UI availability
or full ArcGIS compatibility. The older `GAP_ANALYSIS.md` is a historical planning
document, not the current progress counter.

## Remaining release work

Native desktop interaction, app-workspace quality gates, other browsers, large-data
performance, installation on a clean target OS, online services and new ArcGIS
black-box probes remain separate validation tasks. Existing saved ArcGIS references
can run here; generating new licensed ArcGIS reference output requires the bridge
on the ArcGIS machine.
