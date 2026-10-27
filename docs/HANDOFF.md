# Current handoff

Updated: 2026-10-27T14:30:08+00:00; last agent: Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch main; HEAD 735d4f3, pushed and remote verified
735d4f33b2de46c6b1901257294cc01a7af699a3. User authorizes ongoing commits and pushes.
Do NOT launch another agent, CLI session or coordinator.

## Objective

User requests sustained development, validated commits, pushes and dashboard updates
without repeated “continue” prompts, until unable. Prior Claude run hit usage limit;
recovered unfinished Graphic Buffer from .git/agent-coordinator/20261027T101238-693a3583/.
Basemaps, branding and dock work already landed; do not return to stale requests.

## Completed and pushed during this Codex run

- d66383c: finish inherited Graphic Buffer caps/joins/holes; original ten ArcGIS cases.
- ebc2d2d: implementation-aware dashboard reference coverage and four Python tests.
- cdc23aa: expand to 28 Graphic Buffer cases, erosion/zero polygons, numeric reserved-
  field replacement and correct bevel fallback for over-limit miter corners.
- c3abd4a: per-feature numeric/text distances, case-insensitive fields, correct imperial
  units, numeric null/field-zero skipping, geographic and malformed text safety guards;
  48 Graphic Buffer cases and three CLI refusal tests. Limits in KNOWN_GAPS/README.
- 735d4f3: dashboard retains CI fetches through offline hooks/fetch failure, displays
  fetch timestamp and latest gaps; eight Python tests. JSON/JS syntax checks pass.
All pushed checkpoints have SUCCESS GitHub CI, including latest 735d4f3:
https://github.com/JanSolo99/geoforge/actions/runs/40518084617
Last successful push gate: /tmp/unigis-gb-fields-dashboard-push.log, exit0, 177 Rust
passed, 5 ignored; prior field gate /tmp/unigis-gb-field-check-final.log also passed.

## Current slice: Composite Bands (validated, staged; committing next)

374 reviewed paths staged explicitly, excluding unrelated dirt. New module
crates/unigis-raster/src/composite.rs, lib/registry registration; stats.rs writes
statistics for every band. CLI arcgis_reference.rs now checks band count, all cells
and per-band statistics; corruption/missing-band/missing-metadata regression test.

86 ArcGIS cases: 18 realistic/multiband/grid/type cases, complete 64-pair matrix of
all eight pixel types, four zero-padding cases. Nine tiny input TIFFs under
 testdata/raster/composite_matrix. Generator CASES/RUN/RASTER_OUT, manifest,
cbands_* output sets and scripts/research/composite/* belong to this slice.

Behavior: concatenate all bands, first grid, nearest sampling, full pixel-type
promotion. U32+signed or F32+32-bit int promote F64. Stored pixels compared EXACTLY
for cbands_* (other raster arithmetic retains existing tolerances). NoData comes
from last input; with no source tags padding is zero and tag stays absent. Mixed-
type default NoData when last input lacks a tag still needs probes; recorded gap.
Different CRS/rotated inputs/non-square first cells refused; band selectors and
nondefault environments/extra ArcGIS sidecars unsupported. Evidence/reproduction:
 scripts/research/composite/README.md, probe*.py, matrix*.json, make_matrix.py.
GDAL only creates/inspects files; ArcGIS is behavior oracle. No SDK/decompilation.

## Validation and processes

- First 18-case comparison PASS /tmp/unigis-composite-ref-first.log, then full gate
  PASS /tmp/unigis-composite-check.log (before type/padding follow-up).
- 68 further oracle cases all succeeded /tmp/unigis-composite-matrix-generation.log.
- FINAL scripts/check PASS exit0 /tmp/unigis-composite-final-check.log: 178 passed,
  0 failed, 5 ignored, 79 result groups, eight dashboard tests, fmt and clippy
  workspace/all-targets -D warnings. Includes 86 exact Composite cases and all older
  references, three Graphic Buffer guards, and later-band comparison regression.
- Dashboard --ci refreshed: 277 own names, 304 ArcGIS entries with references;
  generated JSON invariants + node --check /tmp/unigis-dashboard-composite.js PASS.
- No Cargo/probe processes running. No other agent launched. Source frozen and
  staged; only handoff needs restaging before commit.

## Next actions

1. Commit staged Composite Bands slice, push normally (mandatory pre-push gate),
   verify remote HEAD and CI. Dashboard will be dirty after post-commit by design.
2. Probe mixed-type NoData defaults with tagged synthetic matrix inputs and an
   untagged final input; do as separate follow-up, preserving validated checkpoint.
   Equal-type F32 fallback is saved, mixed-type defaults are still unverified.
3. Continue bounded oracle-backed registry work, tested commit/push checkpoints and
   dashboard refreshes. User is sleeping: do not stop after one tool or ask “continue”.

## Durable context / preserve unrelated edits

Graphic Buffer findings: scripts/research/graphic_buffer/README.md. Earlier tool
batch findings: scripts/research/sa_batch/README.md. Read ROADMAP/KNOWN_GAPS before
choosing another tool. Inspect actual ug tools; capture to file rather than pipe to
head (known broken-pipe panic, not fixed). New tools use registry + generator RUN +
saved cases. scripts/check is mandatory pre-push gate; no bypass. Cargo jobs <=3,
no overlapping builds, no local full app CI for root-only edits. Do not recurse into
another coordinator or agent. Post-commit dashboard uses cached CI offline now.

Preserve/exclude unrelated tracked aux.xml changes in testdata/{geoproc,raster},
deleted tracked *.sr.lock files, .worktrees/, NOTES.md, scratch_cf.tif and
scratch_trend_rms.txt.xml. Do NOT read/use NOTES.md (proprietary reverse-engineering
material). No reset, stash, worktree transfer or external messaging performed.
