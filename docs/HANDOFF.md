# Current handoff

Updated 2026-11-08T03:11:40.556206+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`.
Product HEAD `e34da9a85c57a93128021f84c1ab82eb313be823`; this documentation checkpoint follows it.

## Single item: app.unigis-naming — complete and pushed

Continued Claude's accepted CLI dispatch slice `134ab3b4`; no older queue work
resumed. This turn landed the remaining naming implementation as `e34da9a85c57a93128021f84c1ab82eb313be823`.
Normal push origin/main succeeded, exit0, remote verified equal to product HEAD;
`/tmp/naming-push.{log,exit}`, `/tmp/naming-source-remote.txt`.
User authorizes commit/push and one item only; no agents/coordinator launched.

## Completed implementation

- UniGIS/unigis installed names: Linux desktop entry/icon/MIME, macOS app/binary,
  Windows installers/portable/MSIX, release artifacts, package metadata and smoke
  checks. Windows console-subsystem `unigis-cli.exe` plus portable `ug` use the
  existing CLI library. Linux distro packages omit ug to avoid ugrep collisions.
- Linux preferences copied from rustgis/prefs.json into unigis/prefs.json only
  when new preferences do not exist. Old file stays; invalid/new settings are not
  overwritten. Real native app loaded French preferences from this migration.
- Own logo source (`render/src/logo.rs`) drives xtask web-icons/web-pwa, Trunk's
  pre-build hook and downloads icons. Local favicon/touch/PWA/loading images;
  padded opaque maskable icon; generated files included in offline cache/hash.
  Downloads metadata includes a branding digest so logo changes refresh the site.
- User/packaging/agent-install docs, attribution, roadmap and priority updated;
  queue regenerated. External repository/domain/Store identities, MSI UpgradeCode
  and legacy MIME alias retained. Historical published winget manifests remain
  historical. New package names take effect on the next release; no registry
  publication was performed by this code landing.

## Validation of final product

Sequential Cargo with -j3 and CARGO_TARGET_DIR=/home/jansolo/.cache/geoforge-target.
From app unless a root script is named:
- `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=1 cargo xtask ci`: fmt + workspace clippy
  -D warnings PASS; tests stop at six pre-existing engine WASM plugin fixtures
  returning nope (126pass/6fail/1ignore). `/tmp/naming-app-ci.{log,exit}`=1.
  Do not claim full app CI passes; documented in KNOWN_GAPS, unchanged fixtures.
- `cargo test -j3 -p rustgis -p rustgis-cli -p xtask`: PASS3 desktop +1 CLI +6 xtask,
  including migration preservation and icon/cache/mask-safe regressions.
  `/tmp/naming-final-focused.log`; `cargo xtask layers`: PASS, naming-layers.log.
- `cargo check -j3 -p rustgis-web --target wasm32-unknown-unknown`: PASS,
  naming-wasm.log. `cargo build --release --locked -j3 -p rustgis -p rustgis-cli`:
  PASS4m37s, naming-release.log, naming-final-checks.exit=0.
- Root `CARGO_TARGET_DIR=... scripts/check`: PASS exit0, naming-root-check.{log,exit}.
  Normal source push re-ran the protected gate and passed, naming-push.{log,exit}.
  Protected verifier unchanged. Final diff check PASS; workflow YAML, shell/Node
  syntax, packaging XML, desktop entry, MIME database and AppStream checks PASS.
- Full `NO_COLOR=true CARGO_BUILD_JOBS=3 CARGO_TARGET_DIR=... scripts/build-web.sh`:
  PASS, naming-web-build.exit=0; generated worker caches10 files. Actual local
  icons/manifest/cache references checked, 512px logo inspected. Initial NO_COLOR=1
  was rejected by Trunk; environment-only correction passed. Full downloads
  fixture with stub gh/zero-asset release PASS; naming-site-checks.exit=0.
- Portable tar and AppImage built/extracted and ran canonical unigis version/logo
  commands; AppRun targets usr/bin/unigis, no ug in its installed tree. Extracted
  binary hash equals native-tested release. `/tmp/naming-packages{.log,.exit}`=0,
  naming-portable-check/, naming-appimage-check/. Metadata rendered from synthetic
  SHA256SUMS in naming-package-metadata/ (not a real release download).

## Actual native check and limits

Release GUI in isolated XDG_CONFIG_HOME=/tmp/naming-ui-config, data/cache paths
also isolated. French interface and new UniGIS title/logo inspected; old prefs
unchanged and custom migration probe preserved in new file.
`/tmp/naming-desktop.png`, naming-desktop-result.json, naming-desktop.log.
Own PID3153016 stopped via SIGTERM; no relay started, no user settings touched.
An initial fmt command used a root-relative manifest from app and was rejected;
CI's own fmt check passed. One package extraction attempt had a not-yet-created
workdir and started no process; corrected isolated extraction passed.

Debian/RPM tools, macOS signing/notarization and Windows NSIS/WiX/MSIX tools are
unavailable here; those platform installs were not run. Their manifests parse and
release smoke checks were updated. Existing first-argument path precedence makes
`unigis tools` see the repo's tools/ as input; tools was verified from /tmp and
this existing behavior is recorded in KNOWN_GAPS for later requested polish.

## Preservation and next action

Inherited source/agent/reference hashes match `/tmp/naming-inherited.json`;
only generated dashboard refreshed by post-commit hook and remains unstaged.
Inherited raster edits, .claude agent edit, reference script and untracked notes/
worktrees/research remain untouched and uncommitted. No reset/stash/transfers.
Protected checksum: `/tmp/naming-verifier.sha256`. No generated images committed.
No pending product edits or validation for this item. This final operational
checkpoint accompanies the landing; no other queue item was started.
Next queue item: app.gdb-subtypes-relationships, only after coordinator acceptance.
