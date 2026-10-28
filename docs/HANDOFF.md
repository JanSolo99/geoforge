# Current handoff

Updated 2026-10-28T13:36:16.816330+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`;
HEAD9b4dcaf; normal push RUNNING (session/log below). Previous
27ae5ccb66e15b93866103e55ace0094028ea95f PUSHED and verified remote.
User authorizes continuous tested development, frequent commits/pushes/dashboard
updates. Work alone; no agents/CLIs/coordinators. Cargo-j3, sequential builds,
normal hooks, no full local app CI for root edits. Preserve unrelated files below.

## Completed/pushed this long-running session

- e73e223 Line Statistics22 references, F32 local-coordinate clipping and guards.
- a613de7 Copy Raster PAM-placement policy22 controls.
- 48d99ae stale PAM cleanup after no-PAM overwrite; safe native in-place extension.
- 002a79b UInt32/default/tagged NoData48 refs, preserved embedded caches, native
  metadata API and Expand DISTANCE right/bottom statistics workspace. Full gate190.
- 27ae5cc fractional Copy Raster44 refs: ties-even markers, midpoint-up pixels,
  signed range boundaries, raw negative unsigned TIFF tags separate from logical
  statistics masks. Native/GDAL classic+BigTIFF writer override regression.
  Full local gate AND normal push hook PASS191 Rust/5ignored/8Python/fmt/clippy.
  Logs /tmp/unigis-copy-fraction-check.log, /tmp/unigis-copy-fraction-push.log.
  Dashboard--ci/JS syntax/Python syntax/diff checks PASS.

Durable findings/reproducers: scripts/research/copy_raster/{statistics,u32,fraction},
scripts/research/line_statistics, docs/formats/raster-statistics.md.
Do not infer absence of statistics from no PAM:120 references store them in TIFF.
Do not recompute stale caches/counts: COUNT0 and COUNT>savedcells can be intended.
ArcGIS same-path CopyRaster probe failed999999 and damaged scratch pixels; native
safe in-place copy remains an intentional extension, documented separately.

## Active cache follow-up (uncommitted, this session)

11 new cprcache_* cases generated successfully; frozen source TIFF/PAM pairs in
`testdata/raster/copy_cache`. All input bytes unchanged by oracle. Controlled
caches are synthetic format inputs; ArcGIS Pro3.7 is the GP oracle, GDAL reads
files only. Source input copies were frozen BEFORE oracle to prevent mutation bias.

Findings: both cached extrema select embedded/no-PAM output; absent either =>fresh
PAM. If MEAN or STDDEV absent, reset BOTH to0/-1, preserve min/max. MissingCOUNT
=>0.000000, MEDIAN=>nan, skipfactors=>1. Existing values retained, including stale
complete COUNT999, median123, skips2/3, covariance7 and excluded127. Never replace
complete stale fields with freshly calculated pixel statistics.

Edits: CLI main existing cached-copy loop fills paired moments and missing ancillary
fields; reference harness cprcache_ exact prefix; CLI raster_output_statistics
regression compares EVERY field for8 embedded-cache golden cases. Generator,
manifest,11 source/11 reference files+sidecars; research recipes/observations in
scripts/research/copy_raster/cache; ROADMAP/KNOWN_GAPS/formats doc updated.

Validation:
- Native build PASS /tmp/unigis-copy-cache-build.log.
- Generator11 success EXIT0 /tmp/unigis-copy-cache-references.log.
- All11 grid/type/tag/all-cell/core-stat comparisons PASS
  /tmp/unigis-copy-cache-after.log; prior nine-control metadata comparison found
  three moment mismatches, preserved in research/cache/comparison_before.json.
- Initial full `scripts/check` FAILED session3749, /tmp/unigis-copy-cache-check.log.
  Python8/fmt/clippy/reference suite passed. New full-key cache test revealed
  missing default EXCLUDEDVALUES empty field (GDAL GetMetadata hides it). Added
  default empty value; revised gate PASS192 Rust/5ignored/8Python/fmt/clippy.
- Python py_compile and git diff --check PASS. Dashboard--ci and JS syntax PASS.

## Next actions

1. Revised scripts/check PASS192; commit cache slice, normal push, verify remote.
2. Refresh dashboard--ci, extract/check JS; git diff --check; inspect intended diffs.
3. Commit explicit paths for this cache slice; normal push (required hook), verify
   remote SHA. Update this handoff during milestones and before finishing.
4. Follow ROADMAP/KNOWN_GAPS. Unverified cache scope: malformed/nonfinite fields,
   incomplete multiband caches, nondefault environments. New-tool inventory found
   CalculateDistanceBand missing; metadata /tmp/unigis-distance-band-info.log;
   no implementation started. Prefer one concrete verified slice at a time.

## Preserve unrelated files

NOTES.md DO NOT READ/add; .worktrees/ untouched. Inherited scratch
`testdata/geoproc/LineSta_roads1.*` excluded. Own LineSta_roads2 removed earlier.
Post-commit dashboard/handoff dirt expected. No reset/stash/worktree transfers.

Revised gate63483 COMPLETE exit0 /tmp/unigis-copy-cache-check2.log.
Missing EXCLUDEDVALUES empty default added after first gate failure.

Revised cache gate63483 PASS exit0:192 Rust/5ignored/8Python/fmt/clippy.
Every-field test now PASS, including empty excluded-values. Dashboard--ci,
extracted JS node--check, Python syntax and diff checks PASS. Committing cache
slice next, then normal push/remote verification. No Cargo running now.

Private NEXT research8 multiband cache controls completed EXIT0 session84517:
/tmp/unigis-copy-multicache-controls/comparison_before.json. Six routing failures:
ArcGIS requires cached extrema in EVERY band; missing any extrema recomputes ALL
bands to PAM. Native naive geotiff parser mixes keys across bands/domains and
misses single-quoted attributes. Two all-extrema partial-moment cases pass.
Do not change product source until current slice pushed. Next source design must
keep Raster.statistics/type-planning semantics correct: F64 unary cache probe
RUNNING session92106, /tmp/unigis-copy-multicache-types.log.
Potential shared stats_metadata XML parser refactor can fix geotiff cache routing
without new Raster fields/app API changes; verify unary behavior before choosing.
app/AGENTS.md read while assessing API impact; no app edits made.

Cache slice committed9b4dcaf with full gate192 PASS; normal push RUNNING
/tmp/unigis-copy-cache-push.log. No new product edits until push hook completes.
Multiband type oracle session92106 EXIT0: Abs/Negate/Square keep F64 when ANY band
lacks cached extrema, narrow toF32 only for complete caches (single-quoted XML
valid). This supports making Raster.statistics Some(first-band-range) only when
EVERY band has min/max, using shared metadata XML parser with band/domain/quote
handling. No new Raster field/app API change needed; root-only checks suffice.
Next research17 saved refs (8CopyRaster+9unary) and robust reader, after push.

Claude 2026-10-29: Line Statistics implemented (`line_statistics` in interpolate.rs, `ls_*` 9 cases, all
pass; code was swept into Codex's commits). Rules found: value = field weighted by LENGTH inside the circle:
LENGTH = sum(field x length inside), MEAN = length-weighted mean, MAXIMUM/MINIMUM/RANGE/VARIETY/MAJORITY/
MINORITY by length per distinct value; grid = IDW default (extent grown half a cell, round(w/cell)+1);
integer field -> S32 NoData -2147483647 except MEAN/LENGTH (F32). Median not done. Pushing is left to the
running Codex push; do not start a second push while it runs.

Claude 2026-10-29: Surface Parameters again (SLOPE+ASPECT from ArcGIS turned into gradients and regressed on the 3x3
cells): not a fixed linear 3x3 filter (max residual 2e-5 on gradients ~0.03; fitted weights ~ (1,4,1)/12 columns but
with non-zero middle-column weights), only the 1-cell border is NoData, so the neighbourhood is 3x3 and the fit is
nonlinear in the slope/aspect formulas. Not implemented. User asked (2026-10-29) to keep going until ALL ArcGIS
tools exist: ~330 of 2028 tools have ArcGIS reference cases so far; the dashboard (http://100.99.35.95:8765/dashboard)
tracks it. Rule for the long run: keep to probe -> closed-form fit -> registry entry -> reference cases -> commit.

Claude 2026-10-29: Diff and Over (Image Analyst; `diff`, `over` in the raster registry, `diff_*`/`over_*` 14 cases pass):
Diff = first value where it differs from the second, 0 where equal (type from the FIRST operand only: width, floating,
signed-if-statistics-negative); Over = first value, or the second where the first is 0 (background); type = widest of both.

Claude 2026-10-29: Compute Change Raster (single band; DIFFERENCE = to - from, RELATIVE_DIFFERENCE = (to - from)/max(|from|,|to|), 0
where both 0; F32 NoData 3.4e38, no statistics sidecar; `compute_change_raster`, `cc2_*` 10 cases pass). Categorical difference,
spectral distance/angle and multiband inputs not done. Probed ConvertSARUnits, CreateBinaryMask, CreateColorComposite,
SummarizeCategoricalRaster usage only (SAR/multiband inputs needed).
