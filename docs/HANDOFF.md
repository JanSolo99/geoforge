# Current handoff

Updated 2026-10-28T13:36:16.816330+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; branch `main`;
HEAD27ae5ccb66e15b93866103e55ace0094028ea95f PUSHED and verified remote.
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
