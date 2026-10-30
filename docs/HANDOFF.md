# Current handoff

Updated 2026-10-30T11:47:05+11:00; last agent Kimi.
Workspace `/mnt/media/ArcGIS_Linux`, main, HEAD c4384eaa4ed67b446aef6ed0af5af4b247a3f9f7.
User objective: finish the current queue tool, then build the next eligible tool under the queue
rules (ArcGIS references, full gate, scoped commit/push).

## GPX To Features — validated, ready to land

- The two recorded failed coordinator attempts only failed at launch (`--prompt` with `--auto`);
  their logs contain no agent events. GPX edits were already present in the working tree.
- `crates/unigis-tools/src/gpx.rs` parses GPX with roxmltree; registry id `gpx_to_features`,
  parameters `input_gpx_file`, `output`, `output_type` (`POINTS` default or `TRACKS_AS_LINES`).
- POINTS writes tracks, routes, then waypoints as PointZM with ArcGIS shapefile fields. Lines
  writes one single-part PolylineZM per track, joining segments. Four ArcGIS Pro references cover
  both modes and carried desc/cmt/sym metadata. `crates/unigis-cli/src/main.rs` preserves GPX's
  exact compound WGS84/vertical WKT rather than normalizing it to WGS_1984_3D.
- Initial comparator failures were fixed: vertical CRS, single-part lines (the old probe note was
  wrong; actual ArcGIS references and full geometry comparison prove single-part), exact WKT.
- Validation: `cargo test -j3 -p unigis-cli --test arcgis_reference
  tools_match_arcgis_output -- --exact` PASS (all manifest cases; four GPX included).
- Full `scripts/check` PASS, exit 0 (`/tmp/unigis-gpx-check.log`, second run with extended timeout;
  the first reached final tests without failures but hit the 300 s command limit).
- Next: commit/push only GPX files, regenerate queue. Mobile Geodatabase To File Geodatabase has a
  concrete missing-reader defer recorded unstaged; Multipatch to Raster has default/max/min ArcGIS
  probes in `/tmp/unigis-mpr-*.tif` and is the next feasible tool.

## Working tree safety / unrelated edits

Preserve inherited raster (`unary_integer_float`), coordinator/hooks/AGENTS/web and other edits.
Do not read/add NOTES.md or .worktrees. The `umintfloat` generator hunk is unrelated and must not
be staged with GPX. GPX scope is Cargo.lock, unigis-cli/src/main.rs, unigis-tools Cargo/lib/
registry/gpx, the gpx CASES/RUN hunks in scripts/make-arcgis-reference.py, gpx research fixtures/
README, testdata/geoproc/gpx_{sample,sticky}.gpx, manifest and all gpx_* shapefile sidecars,
ROADMAP/KNOWN_GAPS/HANDOFF.
