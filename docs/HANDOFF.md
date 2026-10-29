# Current handoff

Updated 2026-10-29T14:21:21.521699+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD/origin `5a374a2c7285318ca55e23c3f3531ebbebcce811`.
190 scoped paths staged; full gate PASS; ready to commit/push. No builds/probes active.

## Objective and state

User: finish/wrap tool in progress, then build next WORK_QUEUE tool, public ArcGIS
controls, full gate and separate scoped landing. Random Points5a374a2 was already
complete/pushed on startup and not repeated. No unfinished product edits inherited.
This turn implemented Create Spatial Sampling Locations; no incomplete product work.
No agents/coordinators/app edits/web builds or publishing launched.

## Implementation and evidence

- `crates/unigis-tools/src/spatial_sampling.rs`, registry/lib registration:
  polygon RANDOM, STRAT_POLY/STRAT_ID with EQUAL/PROP_AREA/FIELD/PROP_FIELD;
  largest-remainder area/population allocation, spacing and saturation110547.
  No locations110544, invalid counts000531, missing conditional ID000530.
  RANDOM dissolves boundaries and uses one global count/default100, CID1.
  Equal/field counts use CID1-based; proportions use CID0-based. Public FileGDB
  and shapefile controls confirm CID offset is independent of source OIDs.
- SYSTEMATIC reuses existing five-shape tessellation with explicit area units;
  centroid/polygon output, inclusion by centroid/within/intersect, measured
  Shape_Length/Shape_Area, GRID_ID,1-based unfiltered-grid ORIG_FID for centroids.
  Initial refs failed4 non-square cases: public vertices lie on standard FileGDB
  XY grid; native now snaps before measurements/centroids and uses XY tolerance
  for centroid selection. An intermediate FileGDB step is an inference from
  public saved output coordinates/metrics, not private implementation inspection.
  Existing comparison/tolerances kept. Shared crate-private polygon sampler added
  to random_points.rs; existing Random Points code/behavior unchanged.
- Immutable source `testdata/geoproc/ssl_areas.shp` saved once before controls.
  Three disjoint rectangles: areas100/300/200, strata A/A/B, populations2/3/5.
  Do not regenerate. Public recipes/results/precision probe in
  `scripts/research/spatial_sampling/`; README, ROADMAP and KNOWN_GAPS updated.
-23 ssl_* references:19 success/4 expected refusals.2355 prior cases unchanged;
 2378 total. Random/stratified comparisons use schema/CRS/allocation/membership/
  spacing/uniqueness contracts; systematic outputs use existing strict comparator.
- Scope/gaps: polygon inputs only; raster study areas/strata, CLUSTER, H3 and
  count-based bin optimization unsupported. Native seed0 reproducible, not ArcGIS
  stream parity; bounded rejection may underfill. Projected-meter controls verified;
  custom XY resolution/domains/other CRS units, environments/selections, NULLs,
  unusual IDs, Z/M/curves and advanced class metadata unverified. Curves refused.
  No runtime GDAL/SDK dependency. Native failures preserve outputs; public110544
  left an empty shapefile family (DBF0 rows), documented difference.

## Validation

- `uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ssl_*'`
  PASS19 successes/4 expected errors; /tmp/unigis-spatial-sampling-references.log.
- Initial targeted `cargo test -j3 -p unigis-cli --test arcgis_reference --test
  spatial_sampling_safety` failed4 non-square grid cases; corrected precision and
  boundary predicate before final full gate. Initial unused test import removed.
  /tmp/unigis-spatial-sampling-tests.log; safety tests then covered by full gate.
- Independent public ArcGIS reader v2 PASS20 outputs:19 successes plus a positive
  COMPLETELY_WITHIN control (25 SquareMeters,9 cells). Checks schema, allocation,
  constraint/spacing and grid attributes/geometry. Saved native_reader_results.json;
  /tmp/unigis-spatial-sampling-native-v2.{log,json}. Initial v1 outputs/results
  retained separately for precision findings. Do not overwrite or regenerate.
- Final `scripts/check` EXIT0 /tmp/unigis-spatial-sampling-check.{log,exit,pid}:
 258 Rust passed,5 ignored; Python8/fmt/clippy -D warnings/no-default-features PASS.
  Includes largest-remainder unit test,2 CLI safety/source immutability/FileGDB
  allocation/area-weighted occupancy tests, full references and format/engine suite.
  No product changes after final gate start; research/docs only afterwards.
- Python syntax, staged190-file scope/whitespace, old manifest and inherited hashes
  PASS. Generator staged without inherited4-line umintfloat block.

## Next actions

1. Commit190 scoped paths for this tool, then push origin main (fresh full gate
   above; avoid duplicate pre-push gate).
2. python3 scripts/work_queue.py; record actual next item/HEAD/push evidence.
3. Final checkpoint before stopping. No further tool polishing requested.

## Preserved inherited work

Raster local/registry/stats, unary safety/fixtures/research, generator umintfloat,
AGENTS/hooks/agents-remote/coordinator, web publisher RAM guard/docs/service/timer,
queue infrastructure and generated dashboard/WORK_QUEUE remain outside commit.
Untracked testdata/geoproc/parcels.shp.xml was present on startup, also preserved.
/tmp/unigis-spatial-sampling-preserved-hashes.json records inherited tracked hashes;
/tmp/unigis-spatial-sampling-scope.json lists190 scoped paths. Never read/add NOTES.md
or .worktrees; scratch_te scratch. Cargo-j3 max; no overlapping/manual web builds.
