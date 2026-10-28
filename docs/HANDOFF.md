# Current handoff

Updated 2026-10-28T17:17:12.886395+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; branch main; HEAD87338d2 PUSHED, remote verified
87338d26e29670d30064da25512ecece4e6e1df9. Standing objective: continue development,
frequent tested commits/pushes/dashboard updates. Work alone; no agents/other
Claude/Codex CLIs/new coordinators. Cargo-j3, one build at a time. Preserve
NOTES.md/.worktrees (never read/add); explicit-path staging only.

## Completed this session

0eba1e7: finished inherited tessellation area-unit/safety/Foot_US precision work,
31goldens, exactPRJ/ordered rings/IDs and fresh fixture recipe verified.
014c027: uncached integer Abs/Negate/Square retain all bands;21typed goldens,
fresh source recipe verified. Both local/full normal push gates195Rust/5ignored,
8Python/fmt/clippy-Dwarnings PASS, remote SHAs verified.
87338d2: registered unary tools refuse unsupported multiband combinations before
output; shared support predicate, six CLI cases preserve every existing file.
Local+normal push gates196Rust/5ignored,8Python/fmt/clippy PASS, remote verified.
Logs /tmp/unigis-tess-area-{check3,push}.log, /tmp/unigis-intmulti-{check,push}.log,
/tmp/unigis-unary-guard-{test,check,push}.log. Dashboard/node/Python/diff PASS per
slice. Research scripts/research/tessellation/area_units and unary_multiband.
Earlier c5064bc/598f777 remain pushed ancestors; do not repeat external actions.

## Active new slice: floating multiband SquareRoot/Ln/Exp

12private ArcGIS Pro3.7 successes: F32/F64 x cached/uncached x3ops, all2bands/F32.
Negative/zero domains, exponential overflow near F32 max and independent NoData
positions. /tmp/unigis-float-unary-more/profiles.json, observations.json,
make_inputs.py/probe.py; /tmp/unigis-float-unary-more-probe.log. Current guard
baseline refuses12/12 code1/no output (baseline.json). Cached TIFFs equal raw
pair byte-for-byte; ArcGIS caches in PAM only. GDAL only format IO/read-back.

New local.rs shared predicate permits Sqrt/Ln/Exp for floats, dispatcher reuses
existing per-band code. Guard regression moves float refusal cases to still-
unsupported Log2/Sin/RoundUp; cached integer refusals retained. No app/API field
changes. Generator adds12umfloatmore_*; four source TIFFs+two frozen ArcGIS PAMs
in testdata/raster/unary_float_multiband. All six byte-match private sources;
initial hashes in research source_verification.json. ROADMAP/KNOWN_GAPS edited.
New research scripts/research/unary_multiband/floating contains profiles, frozen
caches, recipe, observations/baseline, comparator/README. Inherited post-commit
HTML/HANDOFF dirt; all other active edits new here.

## Running / validation

cargo fmt --all/build PASS; /tmp/unigis-float-more-build.log.
Reference generation EXIT0,12 successes /tmp/unigis-float-more-references.log.
Strict12-case comparison PASS with original AND fresh recipe inputs; artifacts
native_after.txt/recipe_verification.txt. Four TIFFs/two PAMs unchanged; verified
source hashes after oracle. Full scripts/check EXIT0 /tmp/unigis-float-more-check.log:196Rust passed/5ignored,
8Python/fmt/clippy-Dwarnings PASS. Dashboard--ci/JS/Python/diff PASS. No Cargo
running. Feature explicitly staged, commit/normal push outcome still pending.

## Next actions

1. Finish build/goldens; strict12-case research comparator, check any exact pixel/
   NoData/type/stat differences empirically. Check source hashes after oracle.
2. Fresh recipe + comparator with optional input-folder argument; save artifacts.
   Update README, dashboard--ci, JS/Python/diff checks. Full scripts/check-j3,
   including shifted six-case guard regression (expected196Rust, not yet passed).
3. Explicit feature commit, normal push hook, verify remote main SHA; checkpoint.
4. Continue roadmap. Cached integer multiband needs all-band stored ranges; do not
   scan samples as a stale-cache substitute. A possible future approach is global
   cached range aggregation for type planning without changing Raster fields,
   but audit tuple consumers and probe asymmetric/stale ranges first. Other unary
   multiband operators, H3 and nonlinear SurfaceParameters remain gaps.

Next-slice private probe EXIT0:21 successful cached integer controls, all2bands. seven cached typed two-band integer sources
(all six types plus reversed I16), ArcGIS CalculateStatistics +Abs/Negate/Square.
/tmp/unigis-cached-int-multiband/{make_inputs,probe}.py and profiles.json;
/tmp/unigis-cached-int-multiband-probe.log. Root source/manifest/fixtures for that
slice untouched. Audit confirmed read_geotiff currently collects all band extrema
but stores only bands.first() in Raster.statistics. Type-planning consumers use
the tuple as a range; per-band source XML remains independently preserved by
stats_metadata. A global union might fix cached unary without new struct fields;
normal/stale/asymmetric oracle evidence still required before editing that slice.

Range-model prototype ready at /tmp/unigis-cached-int-multiband/prototype_union.py:
GDAL slices each source into1band files, overrides only their stored extrema with
the dataset union, then runs existing native single-band Abs/Negate/Square and
compares each result against the corresponding saved ArcGIS band. This isolates
the hypothesis without changing repository code or overlapping Cargo builds.
Prototype outcome pending; run it after verifying21 oracle successes.
