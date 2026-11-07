# Current handoff

Updated 2026-11-07; last agent Claude.
Workspace `/mnt/media/ArcGIS_Linux`, branch `main`, HEAD `141d901336a74e205f71ab9c15d177a9f783e4ea`.

## Objective and status

No app tool is in progress. The 2026-11-07 sessions (Codex, then Claude) worked only
on agent tooling, which is no longer part of this repository:

- The coordinator moved to its own Git repository, `~/unigis-coordinator` (its README
  covers usage, acceptance checks, model routing and measurements). It drives this
  checkout via `UNIGIS_ROOT` (default `/mnt/media/ArcGIS_Linux`); run state stays in
  `.git/agent-coordinator/`. Start it with `unigis-agents` (terminal) or
  `unigis-coordinator` (browser dashboard on 127.0.0.1:8766).
- Removed from this repo in commit "Move the agent coordinator out of the app
  repository": `tools/agent-coordinator/`, `scripts/agents`, `scripts/agents-remote`. The app's web-publish units moved to
  `scripts/unigis-web-dist.{service,timer}` (`docs/WEB.md` updated).
- Agent skills stay on disk but are git-ignored: `.claude/skills/` (development and
  orchestration procedures) and `.agents/skills/` (Codex discovery links to them).

## Inherited work preserved

Uncommitted changes from earlier sessions were not modified or validated:
`crates/unigis-cli/tests/unary_band_safety.rs`, raster `src/{local,registry,stats}.rs` and
`scripts/make-arcgis-reference.py` (a copy is also in `git stash@{0}`, "pre-pull local edits
2026-11-07"); untracked
`.worktrees/`, `NOTES.md`, `scratch_te/`, `scripts/research/unary_multiband/integer_float/`,
`scripts/web-build-memory.py` and `testdata/raster/unary_integer_float/`.

## One `ug` CLI and app packaging (Claude, 2026-11-07, committed)

- The app's `rustgis-cli` is folded into `ug`: `unigis-cli` is now a library
  (`unigis_cli::main_with`) plus the lean root `ug`; `app/apps/rustgis-cli` builds the shipped
  `ug`, which hands engine subcommands to the library and adds the app's (`commands`, `info`,
  `render`, `logo`, `ask`, `mcp`, `plugins`, `session` = old `run --cmd`). `packaging/package.sh`
  stages the `unigis` desktop binary as `rustgis`, honours `CARGO_TARGET_DIR`, skips `.deb`
  without `dpkg-deb`.
- Build the app with `CARGO_TARGET_DIR=$HOME/.cache/geoforge-target`: on this NTFS drive
  (`ntfs3`) rustc loses temporary files ("failed to copy bitcode", "failed to rename archive").
- Validation: root fmt, clippy (no warnings) and `unigis-cli` tests pass; app fmt, clippy
  `-D warnings`, layers and all test crates pass except plugin tests that share global state and
  fail intermittently in parallel (`rustgis-engine` `plugin_*`, `rustgis-plugins`
  `packages_load…` etc.); they pass alone and with `--test-threads=1`, the change there is
  message text only. Not fixed.

## Next actions and running processes

1. **App priorities first**: `docs/APP_PRIORITIES.md` (the user's basic-functionality goals,
   2026-11-07) is rendered at the top of `docs/WORK_QUEUE.md`, `scripts/lane claim` hands them
   out before tools and `python3 scripts/work_queue.py --next` names the first. In order:
   gp-empty-map, catalog-no-freeze, add-data-browser, basemap-gallery, create-features-pane,
   gdb-domains-subtypes, unigis-naming.
2. Then validate and land the inherited raster changes, then the tool queue.

The coordinator dashboard service runs; the coding daemon is stopped.
