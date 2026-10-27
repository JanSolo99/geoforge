# Current handoff

Updated 2026-10-27T19:03:59+00:00; last agent Codex. Workspace /mnt/media/ArcGIS_Linux;
branch main; HEAD bec6e20, verified origin/main matches via git ls-remote.
User authorizes continuous development, tested commits, pushes and dashboard updates.
Do not launch another agent, CLI or coordinator. Cargo -j3, sequential builds.

## Completed / inherited

Prior run pushed Graphic Buffer48 cases, Composite Bands125 cases, dashboard fixes,
closed stdout handling, and ASCII/Float imports35 cases (bec6e20). Import check and
push gate passed180 Rust tests/5ignored/8Python. Basemaps/branding/docking already landed.
Old handoff incorrectly called importer push pending; remote verifies complete.

## Active work

Inherited UNCOMMITTED raster Define Projection CLI dispatch/helper (main.rs), public
PROJ/EPSG geographic classification table/generator and geotiff.rs metadata fix.
Five successful ArcGIS probes preserve pixels/type/grid/NoData with EPSG
4326/32633/3857/7844/7856, saved research/raster_crs/define_observations.json.
GDAL can repair the old wrong GDA2020 GeoKeys by EPSG lookup: test RAW keys too.
This session added defras_* generator cases/native mapping; all five oracle cases
succeeded. Raw GeoKey/GDAL test and CLI preservation/refusal test added. Scoped CLI
test PASS (two F64 bands, in-place and invalid/aux SRS guards). First scripts/check
stopped on clippy chunks_exact_to_as_chunks in new test; corrected, full gate rerunning.
Logs /tmp/unigis-raster-crs-{generation,scoped,check,dashboard}.log.
ROADMAP/KNOWN_GAPS/research README and dashboard CI cache refreshed.
Latest prior importer CI SUCCESS: https://github.com/JanSolo99/geoforge/actions/runs/40540613972
Dashboard JavaScript node --check and manifest mapping assertions passed.
FINAL scripts/check PASS: 182 Rust passed,5 ignored,0 failed;8 Python,fmt,clippy.
New raw GeoKey/GDAL checks and five manifest cases included.
Exploratory multiband CopyRaster probe currently waiting (session26825), outputs
under /tmp/unigis_cpr_*; /tmp/unigis-copy-bands-probe.log. No source edits yet.
Source review found copy_raster uses input.like and band0 only; next slice should
verify/fix multiband preservation after this checkpoint push.
Support is standard GeoTIFF and validated numeric EPSG; no custom WKT/aux SRS override.

## Next actions

1. Commit and push validated raster projection slice through normal pre-push gate.
2. Finish compact ArcGIS multiband CopyRaster probes and save reference cases.
3. Fix confirmed band loss, run root gate, refresh dashboard, commit/push.
4. Continue bounded native engine slices; checkpoint after validation/commit/push.

## Preserve unrelated files

Exclude testdata/geoproc/raster aux.xml edits, deleted *.sr.lock, .worktrees/,
NOTES.md (DO NOT read/use: proprietary material), scratch_cf.tif,
scratch_trend_rms.txt.xml and importer *.vat.* generated during oracle work.
No running build/oracle processes at startup. Post-commit dashboard HTML dirt expected.
