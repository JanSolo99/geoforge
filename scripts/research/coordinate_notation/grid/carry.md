# DDM/DMS carry for decoded grids

ArcGIS Pro 3.7 formatting was the oracle. `carry_sweep.py` checks 5,412
GEOREF controls across longitude 0/15/151E and 15W, latitude 45N and 33S.
`carry_refine.py` checks 222 further controls around the transition. Both save
results to disk, with only a small bridge acknowledgement; returning the large
JSON directly left the first caller waiting although the worker was idle.
This transport cause is suspected, not established. No bridge code changed.

Observed transitions, identical at all six locations:

| Output component | Last unchanged | First carried |
|---|---:|---:|
| DDM minutes | 59.9939003 | 59.9939004 |
| DMS seconds | 59.993898 | 59.993904 |

The symmetric relative comparison

```
abs(component - 60) <= 0.0001 * (1 + (abs(component) + 60) / 2)
```

predicts all observed transitions, including the original counterexamples.
This is an inferred model matching black-box evidence, not a claim about Esri's
implementation. Apply it to the displayed lowest component only: minutes for
DDM, seconds for DMS. A seconds carry then propagates through minutes to degrees.
The theoretical transition is about 59.993900304985, between the refined probes.

Native formatting is checked against every full oracle string (not just the
carry bit) by `compare_carry_controls.py`: all 5,634 strings match exactly.
Saved evidence is `carry_sweep_results.json`, `carry_refine_results.json`;
`summarize_carry_sweep.py [artifact.json]` reports the bracketing observations.

The implementation is scoped to decoded GARS/GEOREF inputs. Angular-input
formatting retains its earlier policy until its boundary behavior is probed.
Existing 12 angular input cases are kept as regressions. At this milestone,
GARS and projected-grid combinations were refused before writing (ten guard
combinations). The subsequent [GARS follow-up](gars-boundaries.md) enables grid
GARS output and leaves eight projected-grid guard combinations.

## Independent controls and explicit table schema

`make_carry_inputs.py` creates `carry_precision_all_valid.csv` with 48 boundary
controls plus 100 seeded varied geographic cells and minute components (including
zero, near-minute carry, near-degree carry, signs and geographic extremes).
The source contains exact decimal GEOREF minute units; no GDAL coordinate
conversion is used. `make_carry_table.py`, through the ArcGIS bridge, inserts
these strings into a typed memory table (ID LONG, COORD TEXT255), then CopyRows
writes `testdata/geoproc/grid_carry_precision.dbf` with 148 rows.

The explicit DBF avoids an unrelated CSV-reader schema difference. ArcGIS
ListFields on the initial CSV already included virtual COORD_X and COORD_Y
Double fields, and the tool copied them into the output. Adding an invalid row
did not remove those virtual fields. Native CSV reading exposes only ID/COORD.
`probe_csv_coordinates.py` and `probe_csv_coordinates_results.txt` preserve this
finding, which remains in KNOWN_GAPS. No schema comparisons were relaxed.

Twelve new active `ccn_grid_input_*` cases cover DDM_1/DDM_2/DMS_1/DMS_2 on the
ordinary GARS/GEOREF fixtures and this precision table. The six earlier grid
success cases and twelve earlier angular cases remain unchanged.

Reproduce:

```
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/carry_sweep.py
cp /tmp/unigis-carry-sweep-progress.json scripts/research/coordinate_notation/grid/carry_sweep_results.json
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/carry_refine.py
cp /tmp/unigis-carry-refine-progress.json scripts/research/coordinate_notation/grid/carry_refine_results.json
python3 scripts/research/coordinate_notation/grid/make_carry_inputs.py
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/make_carry_table.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ccn_grid_input_*_ddm_*' 'ccn_grid_input_*_dms_*'
python3 scripts/research/coordinate_notation/grid/compare_carry_controls.py
python3 scripts/research/coordinate_notation/compare.py --prefix ccn_grid_input_ --count 18
```

## Source ObjectIDs

The typed table also exposed a missing default output field: ORIG_OID.
`probe_object_ids.py` confirms that DBF, shapefile and FileGDB inputs expose
HasOID=true and append the original row IDs after notation fields (zero-based
DBF/shapefile, one-based in the created FileGDB table). CSV HasOID=false does
not append that field. Results: `probe_object_ids_results.txt`.

FeatureClass now exposes has_object_id metadata (default true); CSV overrides
it to false while retaining synthetic iteration IDs. Coordinate conversion uses
that metadata to append source identifiers. The app's materialized Fc uses the
nonbreaking default; no app files changed. No source-extension or OID-name
heuristics are used. Values outside i32 are refused before writing pending an
ArcGIS 64-bit-ID probe. Existing output field-name collision/id_field behavior
remains outside this slice's verified coverage.

`compare_object_ids.py` checks native output field order and identifier values
against all four saved source-type controls. Recreate its private shape/GDB/CSV
sources with probe_object_ids.py before running the comparator.

## Strict file-oracle precision

The four precision files exposed default GDAL WKT display rounding near integer
coordinates after native FileGDB copies. Disabling OGR_WKT_ROUND in the testkit
text subprocess preserves the stored values. Storage resolution, comparisons
and source controls are unchanged. Evidence and the focused file-IO regression:
[GDAL WKT rounding](../../../../docs/formats/gdal-wkt-rounding.md).

## Final validation

`scripts/check` EXIT 0 on 2026-10-29:207 Rust passed, 5 ignored, 0 failed;
8 dashboard Python tests, fmt and workspace/all-targets clippy -D warnings passed.
Log: `/tmp/unigis-carry-check3.log`. This includes strict FileGDB/shapefile IO,
carry boundary and WKT-precision regressions, full ArcGIS reference comparison,
and ten unsupported-combination output-preservation checks.

Scoped checks: all 5,634 exact native/ArcGIS carry strings; all 18 grid cases
(1,768 rows) with exact schema/text/geometry presence/warning counts and numeric
tolerance 1e-8 deg; all 12 previous angular cases; all 4 source-identity controls.
Logs: `/tmp/unigis-carry-{sweep-native2,grid-native3,angular-native2,object-ids-native}.log`.
Every starting manifest case and all hashed unrelated inherited files remain
unchanged. Python syntax and whitespace checks passed. No commits/pushes.
