# Current handoff

Updated 2026-10-28T12:17:11.094680+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`; `main`; parent HEAD48d99ae PUSHED/verified
48d99ae401c20ac02e9d4a5e5117d5494205c82d. User authorizes continuous tested commits,
pushes and dashboard updates. Work alone; no agents/CLIs/coordinators. Cargo-j3,
sequential builds; mandatory normal hooks; no local full app CI for root edits.

## Completed/pushed this session

e73e223 Line Statistics22 refs/guards; a613de7 Copy Raster PAM-placement22 controls;
48d99ae stale PAM cleanup/no-PAM overwrite regression. These all pushed normally.
Latest published gate188 Rust/5ignored/8Python/fmt/clippy. Earlier logs in /tmp.
Native safe in-place Copy Raster extension retained; ArcGIS scratch same-path probe
failed999999/replaced pixels with NoData. Difference/probes committed, not reproduced.

## Current validated slice (commit/push next)

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

1. Explicit-path commit validated slice, normal push with mandatory hook; verify
   remote SHA; checkpoint completion. No builds/pushes currently running.
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
