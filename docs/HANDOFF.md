# Current handoff

Updated 2026-10-28T18:40:03.782995+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEAD97043a1 PUSHED, remote main verified
97043a1cc01d142feef3f2ab4eea3216cce3e2f8. Standing objective continuous development,
frequent tested commits/pushes/dashboard updates. Alone; no agents/other Claude/
Codex CLIs/coordinator. Cargo-j3, one build at a time. Preserve NOTES.md/.worktrees
(never read/add), explicit staging only.

## Completed this session, all pushed/verified

0eba1e7 inherited tessellation units/safety/Foot_US precision (31refs),014c027
uncached integer Abs/Negate/Square bands (21),87338d2 unary band-loss guard,
9142ac1 floating SquareRoot/Ln/Exp bands (12),97043a1 cached integer bands (36).
First two local+normal push checks195Rust/5ignored, others196Rust/5ignored;
8Python/fmt/clippy-Dwarnings PASS. Fresh recipes/scoped comparisons/dashboard/
JS/Python/diff PASS per slice. Logs /tmp/unigis-{tess-area,intmulti,unary-guard,
float-more,cached-multi}-*.log; cached successful fullcheck3, tess fullcheck3.
Earlier c5064bc/598f777 remain ancestors; no external actions to repeat.

Cached slice uses union of COMPLETE stored ranges, preserves per-band metadata,
promotes nonnegative127 toU8, refuses invalid multiband ranges viaNaN planning
extrema. No Raster fields added; new Output.covariance_statistics policy flag,
no app policy constructors/field consumers (rg audit), no app edits. Direct GP
stats differ from SA Raster.save: empty integer band + out-of-storage marker
triggers stored0 statistics/COV initialization; diagonal sample variance, zero
cross terms even for correlated bands. All36 strict final/fresh-recipe cases PASS,
24source components unchanged. Seven-case guard preserves every existing byte.
Evidence scripts/research/unary_multiband/cached +formats/raster-statistics.md.
Failed check1 caught8 float-output regressions from overly broad stored0 policy;
fixed by restricting to cached integer Abs/Negate/Square with integer OUTPUT.
Fullcheck3/push passed196. Failed check2 was interrupted text-patch rerun.

## Active new slice: floating Log2/Sin/RoundUp bands

12private DIRECT GP controls succeeded, all2bands/F32 across F32/F64 cached and
uncached. Source /tmp/unigis-float-unary-more; new controls
/tmp/unigis-float-unary-next/probe.py and /tmp/unigis-float-unary-next-probe.log.
Raw Root/private inputs have no PAM. Baseline12/12 safely refused before feature.
Research scripts/research/unary_multiband/more (probe/observations/baseline,
comparator template/README). Reuses fourTIFF/twoPAM sources and fresh recipe in
../floating, no new source fixture files. GDAL file IO only, ArcGIS Pro3.7 GP oracle.

New local.rs support predicate permits floating Log2/Sin/RoundUp; existing per-
band math handles them. CLI guard floats move to Log10/Cos/RoundDown, retains
integer SquareRoot/Ln/Exp and malformed cached Abs (7refusal cases). Generator
adds12umfloatnext_* cases. ROADMAP/KNOWN_GAPS edited. No app/API changes this slice.
Inherited post-commit HTML/HANDOFF dirt; all other active edits new here.

## Running / validation

cargo fmt PASS. cargo build-j3 unigis-cli RUNNING22492,
/tmp/unigis-float-next-build.log; ONLY Cargo build.
Reference generator 'umfloatnext_*' RUNNING81716,
/tmp/unigis-float-next-references.log. Do not start full gate until manifest/ref
writes finish. New feature not validated yet.

## Next actions

1. Finish build/generation; more/compare_native.py12cases; inspect any exact
   float/pixel differences empirically, keep standard numerical tolerances clear.
2. Fresh shared floating recipe + comparator input-folder override, source hash/
   no-new-PAM verification; save artifacts. Update research docs/guard pointers.
3. Dashboard--ci/JS/Python/diff; full scripts/check-j3 (expected196Rust, not yet
   passed), own explicit commit/normal push hook/remote main verification.
4. Continue roadmap after reading ROADMAP/KNOWN_GAPS. Integer floating-result
   unary ops, remaining floating operators, stale/empty-source cache semantics
   remain bounded candidates. Keep commits small and checkpoint during work.

Float-next scoped comparison:10/12 exactPASS,2F32Log2 cases differ at89 by1ULP;
F64Log2/Sin/RoundUp exactPASS. /tmp/unigis-float-next-after.log. Native f64log2
and log2f give6.47573328018; GP F32 gives6.47573375702. F32 natural-log base
conversion candidates match that cell. log2_precision.json saves candidates.
Additional private32value x2band GP Log2/Ln fingerprint RUNNING (tool session),
/tmp/unigis-log2-precision/{make_inputs,probe}.py, candidates.json and
/tmp/unigis-log2-precision-probe.log. No new precision code yet; full gate not run
for float-next. Do not weaken tolerances before isolating arithmetic path.

Precision solved:32/32values match F32ln*F32LOG2_E, versus28/32ln-division and
18/32directlog2. Added F32-specific path (F64 unchanged), one32value x2band
source fixture/testdata/raster/unary_log2_precision/input.tif and golden case.
Now13umfloatnext_* refs. Strict13 original/fresh-recipe comparisons PASS,
/tmp/unigis-float-next-after2.log and research/native_after.txt/recipe_verification.
Shared source6hashes unchanged; raw/precision sources have no PAM; precision
file byte-matches private oracle. Full gate RUNNING91778
/tmp/unigis-float-next-check.log (only Cargo); dashboard--ci RUNNING48314.
Research README/ROADMAP/KNOWN_GAPS updated. No next integer-float feature edits.

Next-slice private GP probe RUNNING85922:39 integer-input SquareRoot/Ln/Exp
controls (six types raw/cached plus cached nonpositive I16). Private folder
/tmp/unigis-integer-float-unary/{make_inputs,probe}.py and profiles.json;
/tmp/unigis-integer-float-unary-probe.log. Sources copied from frozen Root inputs
with caches retained only for *_cached copies. No next-feature product edits.
Current float-next JS/Python/diff checks PASS, explicit feature files staged;
full gate still running, no additional Cargo allowed until it finishes.

Final float-next validation: full scripts/check EXIT0,196Rust passed/5ignored,
8Python/fmt/clippy-Dwarnings PASS (/tmp/unigis-float-next-check.log). Strict13
original/fresh-recipe cases all exactPASS, including32value precision fixture.
Dashboard--ci/JS/Python/diff PASS. Ready for explicit commit/normal push.
Next private integer-float GP probe EXIT0:39/39, all2bands/F32. No corresponding
product/manifest edits yet; metadata/precision inspection pending.
