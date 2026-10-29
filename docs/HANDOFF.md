# Current handoff

Updated 2026-10-29T13:55:17.799883+00:00; last agent Codex.
Workspace `/mnt/media/ArcGIS_Linux`, main; HEAD `11dae9ada1afaf5b4204c39854f9bfb43d5481f0`.
Origin/main8583fda before pending push. No Cargo/Trunk/ArcGIS probes running.

## Objective and state

Finish/wrap current tool, then build next WORK_QUEUE tool, public ArcGIS evidence,
full gate, one tool per landing. Extract Data From Geodatabase8583fda was already
pushed on startup and not repeated. Unrelated inherited edits preserved.

This turn deferred two tools with concrete public evidence, separate commits:
-96474aa Generate Definition Query From Selection: file000732, unselected layer
 000346, selected standalone MakeFeatureLayer succeeds and updates/returns layer.
  Shared Context/CLI lacks selection/highlight and mutable definition-query state.
  No map needed in this control despite public docs. Research/probe saved in
  scripts/research/definition_query_selection/.
-11dae9a Generate Rectangles Along Lines:25 controls; common50+50m bend with30m
  along/20m across needs3 adaptively placed rectangles, unlike regular stationing4;
  chain/branch cross-links also absent. Research/source ral_* fixtures saved once
  in scripts/research/rectangles_along_lines/ and testdata/geoproc/. No fitted or
  straight-only implementation landed. WORK_QUEUE_DEFERRED entries committed.

Current tool Create Random Points implemented and fully validated;127 scoped paths
ready to stage/land, then push all3 commits together. No incomplete product edits.
No extra agents/coordinators, app edits, web builds or web publishing.

## Implementation and durable evidence

`crates/unigis-tools/src/random_points.rs` + shared registry entry create_random_points:
planar extent/polygon (holes)/line sampling, point subsets; count/distance fields,
spacing, warning000983 on saturation,000117 on empty; CID/Id, multipoint chunks.
Sources override extent; extent may be coordinates or dataset. Counts per feature
for polygons/lines, global for point subsets. Negative count000963/distance000673.
Multipoint CID0 matches public shapefile observations. Reproducible native SplitMix64
seed0 by default; native seed option. ArcGIS streams/selection identities differ.
Bounded rejection can underfill dense/thin data; cross-part spacing stricter than
ArcGIS. Curves/point-source multipoint output refused. Environment reprojection/
selections/geographic units/ZM/advanced inputs unverified.1million point limit.
ROADMAP/KNOWN_GAPS and scripts/research/create_random_points/README.md record scope.

Immutable rnp_{areas,lines,points} source fixtures saved once before controls.
Initial probe reused disposable input/output basenames and assumed CID for extents;
failed snapshots saved transparently. Repo sources unaffected. Corrected controls
use saved sources/distinct outputs. Fractional GAP0.6 and literal0.6 Meters both
honored without rounding to1, confirmed publicly. Do not recreate fixtures.

16 rnp_* public references (14 success/2 expected errors),2339 prior manifest cases
unchanged,2355 total. Only this tool uses stochastic_points comparisons: exact
schema/CRS/counts/chunks/per-source counts plus membership/subset identity/spacing/
uniqueness. Existing exact reference comparisons/tolerances unchanged.

## Validation

- `uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'rnp_*'`
  PASS14 success/2 expected errors, /tmp/unigis-random-references.log.
- Initial targeted `cargo test -j3 -p unigis-cli --test arcgis_reference` matched15/16;
  only negative-gap code wrong (000963 vs public000673). Corrected before full gate.
- Independent public arcpy reader PASS ALL14 native success outputs against public
  schema/count/constraint/spacing/chunk properties, saved native_reader_results.json;
  /tmp/unigis-random-native.{log,json}. First reader had an ArcPy multipoint iteration
  error; corrected via public geometry JSON, no native outputs regenerated.
  Native outputs retained /tmp/unigis-random-native.
- Final `scripts/check` EXIT0 /tmp/unigis-random-check.{log,exit,pid}:255 Rust passed,
  5 ignored; Python8/fmt/clippy -D warnings/no-default-features PASS. Four new unit
  tests cover uniform occupancy, seeds, polygon holes, line length weighting,
  spacing/saturation/limits. CLI safety protects existing outputs for invalid values.
  Full ArcGIS references and all format/engine suites passed. No product edits
  after final gate began. Python syntax, inherited hashes/old manifest audits PASS.

## Next actions

1. Stage only127 paths in /tmp/unigis-random-scope.json, excluding inherited4-line
   umintfloat generator block. Scope/whitespace audit, commit tool separately.
2. Push origin main (fresh full gate above; avoid duplicate pre-push gate), then
   python3 scripts/work_queue.py and record actual next item/HEAD/push evidence.
3. Final handoff checkpoint; no further Random Points polishing requested.

## Preserved unrelated work

Raster local/registry/stats, unary safety/fixtures/research, generator umintfloat,
AGENTS/hooks/agents-remote/coordinator, web publisher RAM guard/docs/service/timer,
queue infrastructure and generated dashboard/WORK_QUEUE remain outside commits.
/tmp/unigis-rectangles-preserved-hashes.json records inherited tracked hashes.
Never read/add NOTES.md or .worktrees; scratch_te scratch. Cargo-j3 max;
no overlapping builds/manual web publishing. /tmp had5.7GiB available during gate.
