# Current handoff

Updated 2026-11-08T03:04:53.778506+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `134ab3b4`.

## Single item: app.unigis-naming — validated, ready to land

Prior Claude slice (CLI library/desktop dispatch and docs) committed as `134ab3b4`
and accepted by coordinator; no acceptance repeated. No uncommitted item edits
inherited. Basemap/Create/domain tasks also landed; do not resume older chat state.
User authorizes this one item through validation, commit/push. No agents started.
Read root/app guidance, development skill, queue/priorities and current handoff.

Current scope: finish installed/package/artifact UniGIS naming, Windows console
launcher, Linux preferences migration without losing old preferences, generate web
favicon/PWA/touch icons from our render::logo and serve locally. Preserve existing
repository/domain/crate/protocol IDs unless the installation behavior requires a
migration. External package-registry publication is outside this code landing.

Inherited dirty files remain unstaged; hashes `/tmp/naming-inherited.json`,
protected gate hash `/tmp/naming-verifier.sha256`; untracked material untouched.
Implementation now in working tree: packaging scripts, platform manifests/installers,
release smoke commands, CLI console twin, Linux preference migration, generated
web/download icons from original logo code (Trunk hook + xtask web-pwa/icons).
Repository/service/Store/MSI identities retained, legacy MIME alias preserved;
distro packages omit ug to avoid ugrep collision. No other item started.

Validation so far (CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target,
sequential -j3 builds):
- `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=1 cargo xtask ci`: fmt/workspace strict
  clippy PASS; workspace tests stop at the same six engine WASM plugin fixture
  failures (`nope`),126pass/6fail/1ignore. `/tmp/naming-app-ci.{log,exit}` (exit1).
- `cargo test -j3 -p rustgis -p rustgis-cli -p xtask`: PASS; migration,
  dispatch and generated icon/cache/safe-circle tests. `/tmp/naming-final-focused.log`.
- `cargo xtask layers`: PASS, `/tmp/naming-layers.log`.
- `cargo check -j3 -p rustgis-web --target wasm32-unknown-unknown`: PASS,
  `/tmp/naming-wasm.log`.
- Shell syntax, packaging XML, desktop-file-validate, Node syntax checks PASS.
  Homebrew/AUR/COPR rendered using synthetic SHA256SUMS; AppStream validation PASS,
  `/tmp/naming-package-metadata`, `/tmp/naming-appstream.log`.
- Downloads manifest metadata-only run with stub gh (zero assets) PASS;
  `/tmp/naming-mirror-candidate`, branding_digest includes own logo/generator source.
- Initial fmt invocation used root-relative manifest while in app and was rejected;
  CI's own fmt check passed. No root gate/native/package execution claimed yet.

Native release build PASS4m37s, `/tmp/naming-release.log` and
`/tmp/naming-final-checks.exit`=0. Actual GUI opened in French from the legacy
prefs, created new prefs with language/custom probe preserved and old file unchanged.
Screenshot inspected `/tmp/naming-desktop.png`, result
`/tmp/naming-desktop-result.json`; own PID3153016 stopped (SIGTERM), no relay started.
`unigis tools` verified from `/tmp`; the existing repository-root tools/ path
collision was observed and documented in KNOWN_GAPS (not changed in this slice).

Actual Linux portable tar + AppImage built, extracted and executed: canonical
unigis prints version and renders logo; tar retains ug, AppImage's installed tree
omits ug and AppRun points at usr/bin/unigis. `/tmp/naming-packages{.log,.exit}`
(exit0), `/tmp/naming-portable-check`, `/tmp/naming-appimage-check`. Debian tools
and rpmbuild are absent, so those outputs were skipped honestly.

Full Trunk web build PASS (4m30s including wasm-opt), `/tmp/naming-web-build.exit`=0.
Its pre-build hook generated icons; web-pwa then cached10 local shell files,
version0998f8d57121889d. First invocation rejected inherited NO_COLOR=1; retry
NO_COLOR=true passed. Inspected generated512px original logo. PWA manifest/icon
paths/cache list checked against actual output. Full downloads builder with stub
gh (zero-asset release) generated local icons/site successfully;
`/tmp/naming-mirror-build.log`, `/tmp/naming-site-checks.exit`=0.
MIME database validation passed; extracted AppImage binary hash equals the build
checked in the actual desktop GUI.

Root protected `CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target scripts/check`
PASS exit0, `/tmp/naming-root-check.{log,exit}` (format/lint/workspace tests/doc tests).
Protected checksum unchanged. Final staged diff check passed. Changed workflow YAML
also parses cleanly. No product edits followed these checks.
APP_PRIORITIES/ROADMAP describe the finished implementation; queue regenerated
(next is gdb-subtypes-relationships). Only naming item worked on.

Debian/RPM, macOS signing/notarization and Windows NSIS/WiX/MSIX tools unavailable here;
platform manifests checked statically, release workflow smoke checks updated. Do not
claim those platform installs passed. No registry publication performed.

Next: commit the reviewed43 task files, then normal push origin main (pre-push
runs the protected gate again). Record the resulting source commit/remote evidence
and land the final operational checkpoint. No other queue item may be started.
No running build/GUI processes. Inherited source hashes still match;
docs/dashboard.html remains unstaged generated work; no images or protected files
are staged.
