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

Uncommitted app changes from earlier sessions were not modified or validated:
`crates/unigis-cli/tests/unary_band_safety.rs`, raster `src/{local,registry,stats}.rs`,
`scripts/make-arcgis-reference.py`, and the `app/` changes in `git status`; untracked
`.worktrees/`, `NOTES.md`, `scratch_te/`, `scripts/research/unary_multiband/integer_float/`,
`scripts/web-build-memory.py` and `testdata/raster/unary_integer_float/`.

## Next actions and running processes

Next app work: take the next tool from `docs/WORK_QUEUE.md`, or validate and land the
inherited raster changes first. The coordinator dashboard service runs; the coding
daemon is stopped.
