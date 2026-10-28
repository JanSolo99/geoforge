# Current handoff

Updated 2026-10-28T16:47:10.638816+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`; HEAD `0eba1e7` PUSHED, verified
remote main SHA0eba1e7c32a61b3b6f4df756705df87c0fa2d2a0. Standing objective:
continuous development, tested frequent commits/pushes/dashboard updates. Work
alone, no other agents/CLIs/coordinators. Cargo-j3; no overlapping builds.
Preserve .worktrees/ and NOTES.md (never read/add). Explicit-path staging only.

## Completed this session

Recovered inherited tessellation area-unit/safety/foot-WKT edits from interrupted
Codex attempt. Finished strict31-case exactPRJ/ordered-ring/ID comparison, verified
fresh template recipe, dashboard--ci/node/Python/diff checks, committed0eba1e7.
Both full local gate and NORMAL push hook EXIT0:195Rust/5ignored,8Python,
fmt/clippy-Dwarnings. Logs /tmp/unigis-tess-area-{check3,push}.log. Research and
validation artifacts scripts/research/tessellation/area_units. Prior pushed
c5064bc/598f777 remain ancestors; do not repeat their work or external actions.

## Active new slice: uncached integer multiband unary

ArcGIS Pro3.7 oracle:21 controls, six integer types plus reversed I16 band order;
Abs/Negate/Square all preserve2bands. Native baseline drops second band21/21.
Seven typed sources read-back verified. Additional12 I16 cached/uncached controls
show cached planner must combine BOTH stored band ranges; Raster.statistics only
retains band0 extrema. Scanning pixels is not an equivalent stale-cache fix.
Current bounded edit extends existing per-band dispatcher to uncached integer
Abs/Negate/Square. Cached integer multiband remains unresolved; other unary ops
also remain outside this slice. No app sources or public API fields changed.

New edits: crates/unigis-raster/src/local.rs (dispatcher condition/comment),
scripts/make-arcgis-reference.py (+21umrawmulti_* cases), seven TIFF sources in
testdata/raster/unary_multiband, scripts/research/unary_multiband (profiles/recipe,
21observations/baseline and12cached controls/probes/comparator/README).
Inherited post-commit dashboard dirt and this handoff also modified.
GDAL/OGR only supplies file creation/read-back; ArcGIS defines GP behavior.

## Running / pending validation

`cargo build -j3 -p unigis-cli` EXIT0; /tmp/unigis-intmulti-build.log.
`uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'umrawmulti_*'`
EXIT0,21successful; /tmp/unigis-intmulti-references.log. All21 companion native
comparisons PASS with original AND freshly recreated inputs; semantic CRS and
every band checked. Artifacts native_after.txt/recipe_verification.txt. All seven
source TIFFs unchanged/no source PAMs; source_verification.json. Dashboard--ci PASS.
Full `scripts/check > /tmp/unigis-intmulti-check.log 2>&1` EXIT0:195Rust
passed/5ignored,8Python/fmt/clippy-Dwarnings PASS, covering current Rust edit and
21 new goldens. Node JS syntax/Python compile/diff PASS. No Cargo running.
Private probes completed: /tmp/unigis-integer-multiband-{probe,typed}.log;
sources/output profiles /tmp/unigis-integer-multiband/typed. No other workers.
Current Rust edit/new goldens pass both scoped and full validation.

## Next actions

1. Finish build/reference generation; run research/unary_multiband/compare_native.py
   against all21 with exact per-band samples/type/marker/stats/grid/PAM placement.
   Fix observed failures empirically. Verify source fixtures unchanged/no caches.
2. Complete README/recipe verification, ROADMAP/KNOWN_GAPS (cached integer still
   drops bands; do not claim complete integer multiband support), dashboard--ci.
3. Validation complete; explicit-path commit staged, normal push and remote
   verification pending. Do not overlap Cargo with the pre-push hook.
4. Continue another small roadmap slice; cached integer range model, other unary
   multiband paths, H3, SurfaceParameters nonlinear fit, ChangeRaster/BinaryMask
   missing modes remain. Read ROADMAP/KNOWN_GAPS before choosing.

Next slice selected: registered unary operations that still drop later bands
(cached integers and other unary functions) should refuse multiband inputs before
writing any output. Share the supported-combination predicate with the existing
dispatcher, add CLI existing-output preservation coverage, update gaps. Keep this
separate from the now-validated uncached integer fix. Do not mutate product until
its pending commit/push hook completes. No app API field changes required.
