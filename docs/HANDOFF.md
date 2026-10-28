# Current handoff

Updated 2026-10-28T17:01:55.794027+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; main; HEAD014c027 PUSHED and remote verified
014c027f1ff08e252f3cccfe57a901ad6e169cdc. Standing objective continuous development,
frequent tested commits/pushes/dashboard updates. Work alone, no agents/CLIs or
new coordinator. Cargo-j3, no overlapping builds. Preserve NOTES.md/.worktrees;
never read/add those. Explicit-path staging only.

## Completed this session

0eba1e7: finished inherited tessellation area units/safety/Foot_US-WKT slice,
31 ArcGIS cases, exactPRJ/ordered-rings/IDs comparator and fresh fixture recipe.
014c027: uncached integer Abs/Negate/Square preserve all bands;21ArcGIS cases
across six integer types/reversed I16; all samples/type/NoData/stats/grid/CRS/PAM
and fresh source recipe PASS. Seven sources unchanged/no new source PAMs.
Each commit passed full local gate AND NORMAL pre-push hook:195Rust/5ignored,
8Python/fmt/clippy-Dwarnings. Remote SHAs verified. Logs /tmp/unigis-tess-area-
{check3,push}.log and /tmp/unigis-intmulti-{check,push}.log. Dashboard refreshed,
node/Python/diff passed. Earlier c5064bc/598f777 remain pushed ancestors.

## Active new slice: prevent unsupported unary band loss

Actual evidence: twelve cached/uncached I16 oracle controls (earlier research)
require all-band cached ranges; Raster.statistics currently retains band0 only.
Three new F64 SquareRoot/Ln/Exp controls retain2bands in ArcGIS (F32), but native
014c027 returns1. Research scripts/research/unary_multiband/guard_*.{py,json},
private /tmp/unigis-unary-guard-controls and /tmp/unigis-unary-guard-probe.log.
GDAL only format IO, ArcGIS Pro3.7 GP oracle.

New edits local.rs: crate-private supports_multiband_unary predicate shared with
dispatcher; registry.rs refuses unsupported multiband inputs before output.
No public API fields or app source edits. Existing supported paths unchanged.
New CLI tests/unary_band_safety.rs: six cached I16 Abs/Negate/Square and F64
SquareRoot/Ln/Exp requests must fail code1 with clear message and every preexisting
file byte unchanged. Frozen ArcGIS cache at
 testdata/raster/unary_multiband/i16_cached_statistics.xml (separate filename
keeps existing i16.tif uncached). ROADMAP/KNOWN_GAPS/research README updated.
Inherited post-commit dashboard/HANDOFF dirt; all other guard edits new here.

## Running / validation

`cargo fmt --all` PASS.
`cargo test -j3 -p unigis-cli --test unary_band_safety` EXIT0, six refusals
and byte-preservation checks PASS; /tmp/unigis-unary-guard-test.log.
Full scripts/check EXIT0 /tmp/unigis-unary-guard-check.log:196Rust passed/5ignored,
8Python/fmt/clippy-Dwarnings PASS. No Cargo running.
Dashboard--ci EXIT0 /tmp/unigis-unary-guard-dashboard.log; inline JS syntax,
Python compile and diff checks PASS. Explicit guard paths staged, unrelated
files excluded. No oracle workers. All validation complete; guard commit/push pending.

## Concrete next actions

1. Finish focused regression, inspect/fix failure empirically. Then full
   scripts/check sequentially (-j3); expected196Rust if all pass, not yet run.
2. Dashboard--ci, JS syntax/Python/diff checks. Explicit-path guard commit, normal
   push hook, verify remote main SHA. Checkpoint after each milestone.
3. Next development options: probe/implement other unary multiband operators or
   design all-band stored-range model for cached integers. Do not scan pixels as
   a substitute for stale caches. Read ROADMAP/KNOWN_GAPS before choosing.

Next-feature private research started while guard gate runs: four F32/F64 two-band
sources (cached/uncached) at /tmp/unigis-float-unary-more; profiles.json and
make_inputs.py capture exact format recipe. Negative/zero/overflow values and
independent missing-cell positions.12 SquareRoot/Ln/Exp oracle controls EXIT0
/tmp/unigis-float-unary-more-probe.log; all retain2bands/F32. observations.json
and baseline.json saved privately; current guard refuses all12 code1/no output.
Each cached TIFF is byte-identical to its raw pair and has an ArcGIS-created PAM,
so a format-only recreation + frozen PAM restoration is valid. No root source/manifest/fixture
edits for this next feature. Finish guard commit/push before implementing it.
