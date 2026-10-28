# Current handoff

Updated 2026-10-28T13:26:45.604165+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; `main`; HEAD002a79b PUSHED/verified remote
002a79b2d1bcd0a69616f551b00c55d5e7a57e67. User authorizes continuous tested commits,
pushes and dashboard updates. Work alone; no agents/CLIs/coordinators. Cargo-j3,
sequential builds; mandatory normal hooks; no local full app CI for root edits.

## Completed/pushed this session

e73e223 Line Statistics22 refs/guards; a613de7 Copy Raster PAM-placement22 controls;
48d99ae stale PAM cleanup/no-PAM overwrite regression. These all pushed normally.
Latest published gate188 Rust/5ignored/8Python/fmt/clippy. Earlier logs in /tmp.
Native safe in-place Copy Raster extension retained; ArcGIS scratch same-path probe
failed999999/replaced pixels with NoData. Difference/probes committed, not reproduced.

## Completed 002a79b slice (PUSHED)

48 new cpru32_*/cprtag_*/cprnd_* refs;17 U32 snapshots +7 genuine ArcGIS PAM caches.
U32 high default markers signed32; U32 destination rounds negative defaults toF32,
I32/F64 keep signed exact. Source mask stays unsigned. Unrepresentable integer
markers preserve converted source sentinel payloads as ordinary data. General GP
zero-encoding/default format API unchanged. Cached source2^31 system-marker special
PAM placement verified against neighbors/overrides.

IMPORTANT correction: no PAM does NOT mean no statistics.120 reference cases use
embedded TIFF GDAL_METADATA;24 without either store are all-NoData. New native
stats_metadata module reads both stores with per-band PAM overrides, encodes caches,
and computes fresh ones. WriteOptions.gdal_metadata writes main-image tag42112.
CLI preserves cached Copy Raster fields before writing (including in-place), embeds
computed stats for other no-PAM GP outputs, and clears old PAM after successful
TIFF write for both policies (including all-NoData results). Format API still has
no metadata by default. Generic comparison now checks both stores while retaining
PAM placement. Own/GDAL classic+BigTIFF/PAM-priority/band-bound tests added.

Do not recompute stored caches: U32 cached system marker keepsCOUNT5 although6 raw
cells are valid. GDAL cache lackingCOUNT exportsCOUNT0, not recalculated17700.
Expand DISTANCE statistics include n cells RIGHT/BOTTOM of the saved extent; a
750-cell result storesCOUNT891. Symmetric padding failed; window enumeration
uniquely matched right/bottom for3 cases. Sources/research in copy_raster/u32;
durable format findings in docs/formats/raster-statistics.md.

## Validation

- Oracle generation all48 success/exit0; numeric old31failed ->6 cached-stat failures
  ->all48 fully pass after embedded preservation (native_after.txt).
- New metadata tests2 PASS with native+GDAL; CLI overwrite regression PASS including
  all-NoData cleanup, correct new cache, in-place preservation and invalid params.
- Initial stronger full gate found4 stats differences; count-from-pixels/symmetric
  padding approaches were incorrect. CorrectedCOUNT0 and right/bottom workspace.
- FINAL `scripts/check` PASS exit0 /tmp/unigis-copy-nodata-final3-check.log:
  190 Rust passed,5ignored,0 failed;8Python;fmt/all-target workspace clippy-Dwarnings.
  All previous cases, all48 new, and120 embedded-stat cases checked.
- Five focused embedded regressions PASS /tmp/unigis-embedded-regressions-final.log.
- Dashboard --ci PASS /tmp/unigis-copy-nodata-dashboard.log; extracted JS node --check,
  Python py_compile, cargo fmt --check and diff --check PASS.
- Generator expand_classes_dist_3 source corrected to classes_u8 to match its
  existing saved manifest/reference and native test (found landuse drift). No
  reference regenerated for that correction; saved case is already classes_u8.

## Next actions

1. 002a79b committed/pushed with normal hook PASS190 Rust/5ignored/8Python/fmt/clippy;
   remote verified. No builds/pushes currently running.
2. Continue explicit fractional Copy Raster NoData normalization: saved probe
   -9999.5 to I16 expects-10000; native still truncates. Probe .25/.5/.75 and positive/
   negative ties on private files before choosing rounding; source-default and
   explicit-argument paths may differ. Research/nodata_encoding has original cases.
3. Audit legacy generator/native argument drift before regenerating other old
   families; newly added cases use RUN manifests. Metadata domains/stale/partial
   caches, nonfinite/unusual source tags and ancillary metadata remain limited.

## Preserve unrelated files

NOTES.md DO NOT READ/add; .worktrees/ untouched. Inherited geoproc/LineSta_roads1.*
excluded scratch. Own LineSta_roads2 removed. Post-commit dashboard/handoff dirt
expected. No reset/stash/worktree transfers.

## Fractional NoData follow-up COMPLETE, committing now

44 new cprfrac_* golden cases; exact type/grid/tag/all-cell/stored-stat comparisons
all PASS. Markers ties-even, integer pixels floor(value+0.5), rounded signed range
checks, cached normalized-equal markers preserve caches. Negative unsigned fractions
keep raw TIFF tag separately from rounded logical statistics marker and old source
payload. Opt-in WriteOptions.nodata_tag passed through registry/CLI; format default
unchanged. Native/GDAL classic/BigTIFF override regression added. Source/fixtures:
extract.rs/registry.rs/writer.rs, CLI main/reference harness, raster/copy_fraction,
scripts/research/copy_raster/fraction. Documentation/generator/dashboard updated.

Validation on these edits:
- Build PASS /tmp/unigis-copy-fraction-build2.log.
- All44 exact oracle comparisons PASS /tmp/unigis-copy-fraction-after2.log.
- scripts/check PASS exit0 /tmp/unigis-copy-fraction-check.log:191 Rust passed,
  5ignored,0failed;8Python;fmt;all-target workspace clippy-Dwarnings-j3.
- Dashboard --ci PASS /tmp/unigis-copy-fraction-dashboard.log, extracted JS
  node --check, Python py_compile and git diff --check PASS.
- Initial comparison31/39 failed, candidate42/44 passed, raw-tag override fixeslast2.
- Seven extra private controls confirm raw negative default/explicit U8 tags and
  lack of extra raw-argument pixel masking; saved JSON/probes in fraction research.

Next: commit explicit paths, normal push with required hook; verify remote SHA.
Then investigate documented partial/stale Copy Raster caches on PRIVATE files.
No Cargo builds currently running; next push hook runs the gate sequentially.
No agents/coordinator launched. Preserve unrelated files above.
