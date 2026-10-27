# Current handoff

Updated 2026-10-27T19:15:49+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; branch main; HEAD 6812ba6, PUSHED/verified
6812ba6bd183c0b6c9ba0ddf9ed4de1eeaf6e80e. User authorizes ongoing tested commits,
pushes and dashboard updates while sleeping; continue without repeated prompts.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed

Inherited prior run: Graphic Buffer48 cases, Composite Bands125 cases, dashboard
accuracy/cache fixes, closed stdout handling, ASCII/Float imports35 cases (bec6e20).
Importer CI SUCCESS https://github.com/JanSolo99/geoforge/actions/runs/40540613972.
Basemaps/branding/docking already landed; old chat requests stale.

This run: 6812ba6 raster Define Projection and corrected geographic GeoTIFF keys.
Five ArcGIS cases4326/32633/3857/7844/7856 retain cells/grid/type/NoData. Geographic
classification from public PROJ metadata, no raster runtime database dependency.
Raw GeoKey/GDAL and CLI multiband/in-place/refusal safety regressions included.
FINAL scripts/check and normal pre-push gate PASS:182 Rust,5 ignored,0 failed;
8 Python,fmt,clippy. /tmp/unigis-raster-crs-{check,push}.log.
CI pending https://github.com/JanSolo99/geoforge/actions/runs/40566379155.
Research/limits scripts/research/raster_crs/README.md, ROADMAP/KNOWN_GAPS updated.

## Active uncommitted Copy Raster slice

Source evidence: previous ug copy of RGB retained1 band vs ArcGIS3.
extract.rs now converts every band in order and fixes untagged source default
NoData: U8/I8=256,U16=65536,I16=32767,U32/I32=2147483647,F32=3.4e38,F64=1.79e308.
F32->F64 uses double3.4e38. registry.rs enables per-band statistics for multiband
copies, including first/later-band NoData. Existing single-band DEM behavior retained.

24 saved cprbands_* cases all succeeded; generic ArcGIS comparison PASS on all24,
exact every pixel/band plus grid/type/NoData/CRS/stats. Generator, manifest and new
TIFF/TFW/XML references, two tiny testdata/raster/copy_raster inputs, research probes/
observations/make_inputs.py/README. See scripts/research/copy_raster/README.md.
ROADMAP/KNOWN_GAPS and dashboard refreshed. All edits this slice UNCOMMITTED.

FINAL scripts/check PASS exit0 /tmp/unigis-copy-bands-check.log:
182 Rust passed,5 ignored,0 failed;8 Python,fmt,clippy. Every24 case included.
Dashboard JS node --check and git diff --check PASS. Commit/push next.
No other build/oracle processes. Research probes finished. git diff --check PASS.

## Next actions

1. Finish full root gate; record exact result, refresh CI/dashboard and JS check.
2. Stage only Copy Raster/docs/dashboard/reference paths, commit/push normally.
3. Next bounded slice:16 compact downcast probes saved copy_raster/downcast_observations.json
   and probe_downcast.py. F64->F32 NoData Infinity matches new native behavior.
   F32/F64->I16 keep3.4e38/1.79e308 tags; current integer tag writer clamps to i64.
   Fix/test huge tags; normalize float output NoData before stats (I32/U32 marker
   rounds to2147483648). Native red probe confirms exactly4 of16 mismatches:
   U32/I32->F32 counts6 vs ArcGIS5, F32/F64->I16 tags i64MAX vs3.4e38/1.79e308.
   Evidence research/copy_raster/downcast_native_before.txt; /tmp/unigis-copy-downcast-red.py.
   Generate saved cases and verify before further commits.
4. Continue tested native engine slices; checkpoint after milestones.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No reset/stash/worktree transfers. Root-only changes: no local full app build.
Pre-push scripts/check mandatory; post-commit regenerates dashboard HTML (expected dirt).
First full-array Copy Raster probe stalled on a large response; only its own worker
cancelled; compact reruns completed. Oracle evidence always ArcGIS; GDAL file IO only.
