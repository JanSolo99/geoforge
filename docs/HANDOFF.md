# Current handoff

Updated: 2026-10-23T04:36:28+00:00; last agent: Codex.
Directory: `/mnt/media/ArcGIS_Linux`.
Branch: `codex/unigis-docking-and-branding`.
HEAD before requested commit: `34695ef401afe721e0c3ab432de429c136b8760a`.

## Current objective and status

User explicitly requested a new Codex branch, commit and push for the existing work.
On 2026-10-23 user directed "push now", authorizing cancellation of the slow full
app test build and proceeding with completed checks. Branch exists; commit/push next.
All 199 reviewed paths are now staged. Staged whitespace check passed and
staged path set exactly matches the manifest, with no worktree/private-log paths.
This checkpoint will be staged before commit.
All 199 prepared file hashes matched at startup. No source changes were lost,
transferred between worktrees, reset or stashed. `.worktrees/` is excluded.

Current turn: read required instructions/status/diffs, created branch, reviewed
prepared scope, passed root precommit gates, and started the full app gate.
Only new content change this turn is coordinator README clarification: `/cli`
opens native UI but does not itself change resumed permissions; `/permissions`
can enable specific approval requests while retaining workspace sandboxing.

## Implementation being committed (inherited from previous turns)

- Dockable pane groups at left/right/bottom, tabs, drag/drop previews, auto-hide
  and saved layouts (`app/crates/ui-egui/src/dock.rs`, UI integration). Fixes for
  tab order, context-menu persistence, scrolling and active-tab indicator clipping.
  Real pointer-drag, layout model and preference restore regression tests.
- Unified ArcGIS toolbox/toolset tree, verified replacements for overlapping tools,
  preserved aliases/defaults and selection on renamed layers
  (`app/crates/analysis/src/{lib.rs,tools.rs,legacy.rs}`, geoforge wrapper crate).
  Corrected stale test expectations and compared EIGHT-neighbor regions to engine.
- UniGIS display branding throughout app/web/PWA/assistant/translations/packaging,
  Python/CLI descriptions and docs. Stable package names, binaries, tool ids,
  Python exception names, formats, credential/settings/cache paths and URLs kept.
- Updated roadmap, known gaps, README/design and all seven local screenshots.
  Screenshot scripts no longer click unavailable city.gdb. Filenames stay stable.
- Shared Claude/Codex coordinator (`scripts/agents`, tools/agent-coordinator),
  AGENTS/CLAUDE workflow guidance and this handoff. Coordinator code inherited;
  it has five offline integration tests passed in its original setup session.

Recovery evidence: Claude’s agents `a6ed447ac536dd96e` (docking) and
`add2aa420c0ca26f5` (unified catalog) stopped on session limits. Their logs are under
`/home/jansolo/.claude/projects/-mnt-media-ArcGIS-Linux/8cc37833-14c8-4e27-9f20-9eec36aff1a8/`.
Rebrand/fixes/validation were completed by Codex afterward.

## Current precommit validation

All Cargo builds/tests are sequential, at most 3 jobs.

- Root `cargo fmt --all && cargo clippy --workspace --all-targets -j3`: passed
  (`/tmp/unigis-precommit-root-clippy.log`).
- Root approved `cargo test --workspace -j3`: passed, 168 tests, zero failures,
  5 ignored (`/tmp/unigis-precommit-root-tests.log`).
- App approved `env CARGO_BUILD_JOBS=3 XDG_CACHE_HOME=/tmp/unigis-precommit-cache cargo xtask ci`:
  **cancelled at user request** after more than 33 minutes,
  `/tmp/unigis-precommit-app-ci.log`. Formatting and full workspace clippy with
  `-D warnings` passed; full workspace tests were still building, with no test
  results. Full test suite and subsequent layer gate did not complete.
- `git diff --check`: passed. Git author configuration is available.

Previous scoped validation (not the full precommit gate): 281 distinct native tests
passed, 31 local-TCP tests were sandbox-blocked and filtered, 1 ignored. Clippy for
changed packages/all targets, final wasm web check, layer check, root CLI check,
packaging XML/plist/PWA, 15 translation catalogs and 6 Python AST checks passed.
These results are recorded in `/tmp/unigis-verified-tests.log`,
`/tmp/unigis-gdal-tests.log`, `/tmp/unigis-clippy.log`,
`/tmp/unigis-ui-final-clippy.log`, `/tmp/unigis-wasm-final.log`,
`/tmp/unigis-cli-check.log` and `/tmp/unigis-layers.log`.

Screenshots rendered and inspected using existing software Vulkan:
`VK_DRIVER_FILES=/usr/lib/chromium/vk_swiftshader_icd.json WGPU_BACKEND=vulkan cargo run -q -j3 --features rustgis-ui-egui/geoforge -p rustgis-ui-egui --example ui_shot -- ../docs/screenshots/scripts/dock.jsonl`
then built executable with tools/form/map/dark/browser scripts. Logs:
`/tmp/unigis-screenshots.log`, `/tmp/unigis-other-screenshots.log`. No software
installed; Windows/macOS installer labels structurally checked, not built.

## Permissions finding

User was already in native CLI; earlier agent repeatedly directing them to `/cli`
was incorrect. Native UI can still have approval_policy=never. User has now enabled
approval review, and protected Git operations and test execution succeeded via
`require_escalated`. No launcher/config changes or sandbox bypass were made.

The 31 TCP tests use local 127.0.0.1 mock servers (downloads/services/S3/STAC etc.)
and MCP relay checks (authentication, timeout, cleanup and command delivery).
Previous failures occurred before those behaviors ran. They were not deleted or
permanently disabled. The approved full app test attempt was cancelled before
test results; these 31 tests remain unverified.

## Next actions

1. Commit the reviewed staged 199-file scope on current Codex branch and push to
   origin with upstream tracking. User explicitly authorized both and waived
   waiting for full app CI. Exclude `.worktrees/` and private runtime logs.
2. Record actual commit/push outcome in this handoff and checkpoint it.
3. Future validation: finish `cargo xtask ci` (full tests and layers), including
   the 31 local-TCP tests. Prior scoped and root checks above passed.
4. Future docking work: floating windows, arbitrary split groups and attribute
   table redocking are tracked in KNOWN_GAPS. No unrelated follow-up started.

Full CI was terminated at user request; no agents/coordinators launched.
