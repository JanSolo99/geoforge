# Current handoff

Updated 2026-10-28T18:08:54.525150+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEAD9142ac1 PUSHED, verified remote
9142ac1de100be9117a44b78cdf5c2989e844a7e. Objective continuous development,
frequent tested commits/pushes/dashboard updates. Work alone, no agents/other
Claude/Codex CLIs/coordinator. Cargo-j3; only one build. Preserve NOTES.md and
.worktrees (never read/add), explicit-path staging only.

## Completed this session, all pushed and remote verified

0eba1e7 inherited tessellation units/safety/Foot_US WKT:31 refs+fresh recipe.
014c027 uncached integer Abs/Negate/Square bands:21refs+fresh recipe.
Both local+normal push gates195Rust/5ignored,8Python/fmt/clippy PASS.
87338d2 unary unsupported-band guard:6file-preservation scenarios.
9142ac1 floating multiband SquareRoot/Ln/Exp:12refs+fresh recipe/domain/overflow.
Both local+normal push gates196Rust/5ignored,8Python/fmt/clippy PASS.
Dashboards/JS/Python/diff PASS per slice. Logs /tmp/unigis-tess-area-{check3,push},
/tmp/unigis-{intmulti,unary-guard,float-more}-{check,push}.log.
Earlier c5064bc/598f777 remain pushed ancestors. No external actions to repeat.

## Active cached integer bands / GP statistics slice

36goldens umcachemulti_* across six integer types, reversed I16, positive127,
all-overflow U32, correlated3band and single-band overflow.12source TIFFs+12frozen
ArcGIS CalculateStatistics PAMs: testdata/raster/unary_cached_multiband. All24
components byte-match original private controls and are unchanged by generation.
Research scripts/research/unary_multiband/cached contains profiles/recipes,
observations, failed union prototype, GP controls and successful native artifacts.
GDAL only format IO, ArcGIS Pro3.7 GP oracle; no runtime GDAL or SDK work.

Source edits: geotiff.rs unions complete stored extrema for common type planning;
single-band tuple behavior preserved. Invalid complete multiband ranges expose
NaNs to retain float cache presence; integer unary guard refuses them. lib.rs
Raster documentation changed, no Raster fields added. local.rs supports finite
cached integer ranges and promotes nonnegative signed8 endpoint127 toU8.
Stats metadata tests expect union1..40, per-band source metadata stays unchanged.
Seven-case CLI safety regression retains unsupported operators and adds malformed
later-band refusal with every existing file byte preserved (scoped PASS).

Critical GP finding: SA Raster.save stats differ from direct GP execution. An
entirely missing integer result band with an out-of-storage NoData marker causes
GP to recompute statistics on stored zeros across ALL bands, including single-
band outputs. GP initializes covariance vectors to sample variance on diagonal,
zero elsewhere, even for correlated bands. Source CalculateStatistics can have
real cross-covariances and those are preserved. Registry now normalizes missing
pixels to stored0 for that condition and sets new Output.covariance_statistics
policy flag; CLI writes appropriate PAM via stats.rs. New public Output flag;
app has no Output policy constructors/field users (rg audit), no app edits.

Initial union prototype39/42PASS exposed127/empty-stat mismatches. First36-phase
attempt30comparison28PASS: SA-only empty-stat policy incorrect for GP. GP probes
proved COUNT6/COV recomputation; 3-band control disproved true cross-covariance
for this initialized GP cache. All corrected. Failed logs preserved:
/tmp/unigis-cached-int-union-prototype.log, cached-multi-after{,2}.log.

## Validation / running

fmt/build3 PASS (/tmp/unigis-cached-multi-build3.log).
Reference generators EXIT0:30+6cases (/tmp/unigis-cached-multi-{,extra-}references.log).
Strict36-case original AND fresh-recipe comparisons PASS including covariance;
/tmp/unigis-cached-multi-after4.log and research/native_after.txt/
recipe_verification.txt. Seven-case focused guard PASS (before covariance flag,
full gate covers final code). Python/diff passed before latest docs; JS pending.
Full scripts/check RUNNING50703 /tmp/unigis-cached-multi-check.log (ONLY Cargo).
Dashboard--ci RUNNING22543 /tmp/unigis-cached-multi-dashboard.log. No oracle workers.

## Next actions

1. Finish full gate; inspect/fix any regressions (expected196Rust, not yet passed).
2. Finish dashboard/JS/Python/diff checks; explicit source/docs/research/36refs/
   24sourcecomponents commit, normal push hook, verify remote main SHA.
3. Continue roadmap after this slice. Other floating multiband functions or
   integer SquareRoot/Ln/Exp are bounded follow-ups. Stale/partial/empty-source
   cache semantics need probes; invalid ranges remain safely refused. Read
   ROADMAP/KNOWN_GAPS before choosing. Keep checkpoints current.

Next-feature private GP probe RUNNING: /tmp/unigis-float-unary-next/probe.py,
/tmp/unigis-float-unary-next-probe.log, shell session in tool output. Reuses four
private F32/F64 cached/uncached sources from /tmp/unigis-float-unary-more;
12Log2/Sin/RoundUp controls. No next-feature product/manifest edits. Root and
private raw floating inputs have no PAMs (verified), preserving uncached controls.
Finish current cached integer commit/push before enabling new operators.

Full gate50703 EXIT101:8 prior single-band integer-input FLOAT-output unary cases
(Exp/Exp2/Exp10/etc) incorrectly normalized to0. GP stored-cell policy too broad;
type_range_of assumes integer. Fixed by explicitly restricting Abs/Negate/Square
with integer OUTPUT. No goldens weakened. /tmp/unigis-cached-multi-check.log.
An exact-text patch initially failed after fmt; stopped that unchanged rerun
(check2.log, exit143), applied verified patch, revised gate pending.
Next private Log2/Sin/RoundUp probe EXIT0,12/12 all2bands/F32. No next-feature
product or manifest edits. Preserve those private controls for next slice.

Revised full gate RUNNING21346 /tmp/unigis-cached-multi-check3.log, only Cargo
process. Actual narrowed condition verified in registry.rs before starting it.
Dashboard--ci PASS; 36-scope/fresh-recipe/covariance comparisons passed before
scope restriction (same integer Abs/Negate/Square path); full gate verifies fix.
Next private GP probe Log2/Sin/RoundUp finished12/12, all2bands/F32;
/tmp/unigis-float-unary-next-probe.log. No next-feature repo edits yet.

Final milestone: revised full check3 EXIT0,196Rust passed/5ignored,
8Python/fmt/clippy-Dwarnings PASS. Scoped36 covariance-aware comparison rerun
on final restricted-policy binary PASS /tmp/unigis-cached-multi-after5.log;
native_after.txt updated. JS/Python/diff PASS.217 intended files staged, unrelated
NOTES.md/.worktrees excluded. Cached slice ready for commit/normal push. No Cargo
or oracle workers running. Next float Log2/Sin/RoundUp12controls already successful
but no next-feature repo changes yet.
