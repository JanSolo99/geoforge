# Current handoff

Updated: 2026-10-27T13:47:58+00:00; last agent: Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch main; HEAD `ebc2d2d` (dashboard checkpoint), follows Graphic Buffer `d66383c`.
Push COMPLETED, exit 0: origin/main advanced `526a0f2..ebc2d2d`.
Pre-push Python/Cargo gates passed; log `/tmp/unigis-overnight-push1.log`.

## Objective

User explicitly requests sustained development, validated commits, pushes and dashboard
updates without repeated prompts, until unable to proceed. Do not launch another agent
or coordinator here. Prior run hit Claude usage limit; recovered request and relevant
Graphic Buffer events from `.git/agent-coordinator/20261027T101238-693a3583/`.

## Current slice: Graphic Buffer and dashboard accuracy

Inherited unfinished source/fixtures for Graphic Buffer failed five reference cases:
round points were too dense; piece unions left gaps in tight bends and bad polygon holes.
Codex replaced that implementation with native styled `geo` offsets via
`crates/unigis-geometry/src/buffer.rs::graphic_buffer_geometry` and the registry wrapper
`crates/unigis-tools/src/graphic_buffer.rs`. All TEN gb_* cases now match, without raising
existing tolerances (0.2 shape, 0.0001 relative area). Unsupported parameters are refused.
Limits/source documentation: `scripts/research/graphic_buffer/README.md`.
Relevant source/fixture edits inherited: tools lib/registry registration, generator CASES/
RUN line, manifest, ten gb_* shapefile sets and buffer classification in CLI comparisons.
All belong to the authorized interrupted slice; review/stage them explicitly.

Dashboard fixes in `scripts/dashboard/build.py`: exclude Param names, discover external
registered constants, require implementation plus successful reference case for green
coverage. Green means saved reference coverage, explicitly not full compatibility.
Four offline tests (`scripts/dashboard/test_build.py`) pass. Scanner matches all 134
names listed by current `ug tools`. Rebuilt `docs/dashboard.html --ci`: 276 own names,
303 ArcGIS entries with reference coverage, 56 implemented without successful references;
ArcGIS entries repeat across toolboxes, so those counts differ. Eight recent CI rows loaded.
Dashboard tests are now wired into scripts/check and GitHub CI; dashboard commit next.

## Follow-up parameter batch (validated; committing/pushing)

18 gb_more_* cases now generated and ALL 28 Graphic Buffer reference cases pass
(`/tmp/unigis-gb-miter-overlap.log`, exit 0). Supports negative/zero polygon distances,
reserved numeric field replacement, round point radii 30/1000, and miter limits
1/1.25/2/10 including acute corners. ArcGIS bevels over-limit miters; native clipping
was incorrect. Bevel base plus overlapping unioned miter patches fixes this without
changing tolerances. Detailed findings: `scripts/research/graphic_buffer/README.md`.
Changes are uncommitted in geometry/tools, generator/manifest, gb_more_* outputs and
new graphic_buffer_angles / graphic_buffer_reserved input shapefiles.
Final wrapper warnings now match observed 000636/000117; full `scripts/check` PASSED exit 0: 174 passed, 0 failed, 5 ignored in 78 result groups,
plus four dashboard tests, fmt and clippy -D warnings. Log `/tmp/unigis-gb-expanded-check.log`.
All 137 slice paths reviewed/staged; unrelated dirt excluded. No running build.
Exploratory field probes completed (`/tmp/unigis-gb-field-research.log`): numeric/text
distance fields, case-insensitive lookup and numeric null skipping supported by ArcGIS;
zero numeric fields skip, negative points fail 000109. Malformed text has inconsistent
geometry/BUFF_DIST behavior, so refuse rather than guess. No field support edited yet.
A constants/unit probe is running session 15065, `/tmp/unigis-gb-unit-research.log`.
Both pushed checkpoint GitHub CI runs completed SUCCESS:
https://github.com/JanSolo99/geoforge/actions/runs/40512024122
https://github.com/JanSolo99/geoforge/actions/runs/40512020282

## Validation

- Inherited baseline reference run FAILED 5 cases:
  `/tmp/unigis-graphic-buffer-reference.log`.
- Native offset pass initially failed 2 round-join area cases:
  `/tmp/unigis-graphic-buffer-native-offset.log`.
- Finer line/polygon join sampling, point circle sampling retained: reference harness
  PASSED, including all ten new cases, `/tmp/unigis-graphic-buffer-round-refinement.log`.
- `python3 -m unittest discover -s scripts/dashboard -p 'test_*.py' -v`: 4 passed.
- `scripts/check` PASSED, exit 0: formatting, workspace/all-targets clippy -D warnings,
  full workspace tests. 174 passed, 0 failed, 5 ignored, 78 result groups.
  Log `/tmp/unigis-graphic-buffer-check.log`. This gate covers pushed `ebc2d2d`, not the current follow-up edits.
- `git diff --check` passed. Both checkpoints pushed and remote SHA verified.

## Next actions

1. Commit staged expanded Graphic Buffer slice; dashboard --ci refreshed.
2. Push normally (mandatory root pre-push gate); verify remote HEAD.
3. Continue bounded oracle-backed development; consider Graphic Buffer distance fields
   and geographic safety next. Keep checkpointing and pushing validated slices.

## Durable context / preserved work

Previous tool-batch observations moved out of the handoff to
`scripts/research/sa_batch/README.md`; current gaps/overall plan remain in docs/KNOWN_GAPS.md
and docs/ROADMAP.md. New tools use registry + generator RUN + saved ArcGIS cases, not
new clap/test branches. `scripts/check` / .githooks/pre-push is the push gate; core.hooksPath
is already .githooks. Post-commit regenerates dashboard HTML, leaving it dirty by design.
Use public docs and black-box ArcGIS ONLY; do not read/use proprietary reverse-engineering
material in unrelated untracked NOTES.md.

Preserve/exclude unrelated aux.xml changes in testdata/{geoproc,raster}, deleted tracked
*.sr.lock files, .worktrees/, NOTES.md, scratch_cf.tif and scratch_trend_rms.txt.xml.
The old doc push-status handoff was superseded by Claude's later committed work. No reset,
stash, worktree transfer, external messaging or other agent/coordinator launch performed.
