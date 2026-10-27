# Current handoff

Updated 2026-10-27T19:28:28+00:00; last agent Codex.
Workspace /mnt/media/ArcGIS_Linux; branch main; HEAD cd084e6, PUSHED/verified cd084e61bb9a5b6b774b70b78a4986a9a69155b0.
Normal pre-push gate PASS /tmp/unigis-copy-bands-push.log. User authorizes ongoing tested commits,
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

## Copy Raster slice committed cd084e6

Source evidence: previous ug copy of RGB retained1 band vs ArcGIS3.
extract.rs now converts every band in order and fixes untagged source default
NoData: U8/I8=256,U16=65536,I16=32767,U32/I32=2147483647,F32=3.4e38,F64=1.79e308.
F32->F64 uses double3.4e38. registry.rs enables per-band statistics for multiband
copies, including first/later-band NoData. Existing single-band DEM behavior retained.

24 saved cprbands_* cases all succeeded; generic ArcGIS comparison PASS on all24,
exact every pixel/band plus grid/type/NoData/CRS/stats. Generator, manifest and new
TIFF/TFW/XML references, two tiny testdata/raster/copy_raster inputs, research probes/
observations/make_inputs.py/README. See scripts/research/copy_raster/README.md.
ROADMAP/KNOWN_GAPS and dashboard refreshed. Slice committed cd084e6.

FINAL scripts/check PASS exit0 /tmp/unigis-copy-bands-check.log:
182 Rust passed,5 ignored,0 failed;8 Python,fmt,clippy. Every24 case included.
Dashboard JS node --check and git diff --check PASS. Normal push gate running.
No other build/oracle processes. Research probes finished. git diff --check PASS.

## Next actions

1. FINAL downcast scripts/check PASS exit0:183 Rust,5 ignored,0 failed;8Python,fmt,clippy.
   /tmp/unigis-copy-downcast-check.log includes all16 cases and new large-tag regression.
   Native after-check /tmp/unigis-copy-downcast-after.log matches all16 previously4 failed.
2. Record results, refresh dashboard/JS check, stage only downcast paths, commit/push.
3. Frequency tools research (not implemented): fifteen default/multiband/NoData probes
   and eight count boundaries succeeded. /tmp/unigis-frequency-{probe,boundary-probe}.log.
   Counts1/2/3/4/15/16/255/256 give ND16/3/16/15/16/-128/256/65535;
   typesU8 except16 I8 and256 U16. Precise float-comparison probe session28958
   /tmp/unigis-frequency-precision-probe.log. Save scripts/evidence durably before editing.
4. Continue tested native engine slices. Tiny tagged CopyRaster stats/U32 export gaps
   documented in KNOWN_GAPS; no guessed thresholds or fixes yet.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* from prior oracle work.
No reset/stash/worktree transfers. Root-only changes: no local full app build.
Pre-push scripts/check mandatory; post-commit regenerates dashboard HTML (expected dirt).
First full-array Copy Raster probe stalled on a large response; only its own worker
cancelled; compact reruns completed. Oracle evidence always ArcGIS; GDAL file IO only.

Additional research during push (no implementation changes): eight tagged CopyRaster
probes /tmp/unigis-copy-tagged-probe.log show tiny single-band NoData copies do get
statistics (unlike existing large DEM-with-NoData), U32 sentinel export signed wraps.
General policy unresolved; preserve evidence and do not guess thresholds.
Future frequency tools absent in actual ug inventory. Official Esri current Equal/
Greater/Less Than Frequency docs consulted; preliminary15 black-box probes running
session2708 /tmp/unigis-frequency-probe.log, script /tmp/unigis-frequency-probe.py.
No frequency source edits; finish downcast slice before choosing new tools.

## Active downcast slice (validated; committing next)

extract.rs normalizes float output NoData before stats; F32 default uses double3.4e38
for non-F32 output. geotiff.rs preserves integer NoData tags outside i64 range rather
than saturating. Raw GeoTIFF/GDAL regression added for3.4e38 and +/-1.79e308.
16 cprcast_* CASES/native exact comparison added; oracle generation session73517
/tmp/unigis-copy-downcast-generation.log. ROADMAP/KNOWN_GAPS/README updated.
Tiny tagged-statistics and U32 signed-export gaps now explicitly documented.
Next complete root gate, refresh dashboard, commit/push, then consider frequency tools.
Frequency boundary probes running session99851 /tmp/unigis-frequency-boundary-probe.log.

Frequency32 successful probe results/scripts now durably saved in UNTRACKED
scripts/research/frequency/ (baseline15,boundaries8,precision9, README with sources).
Do NOT stage these with the downcast commit; keep subsequent implementation separate.

Frequency broadcast18 probes complete; results /tmp/unigis-frequency-broadcast-probe.log
show MULTI_BAND does NOT broadcast a single comparison band: later output bands all
NoData; extra comparison bands truncate to value-band count (2vs3 outputs2).
Do not implement standard broadcasting assumptions; inspect full results first.
