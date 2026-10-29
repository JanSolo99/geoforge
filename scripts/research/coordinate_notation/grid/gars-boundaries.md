# GARS grid-input boundary arithmetic

ArcGIS Pro 3.7 black-box controls now isolate the decoder and encoder paths.
No Esri binaries/SDK or GDAL geoprocessing algorithm was used. GDAL only reads
stored file fields for comparisons.

## Evidence and models

`probe_gars_numeric.py` takes the old 240 GARS holdout cells and supplies numeric
DD_2 longitude/latitude as rational corners, the inherited decoder's arithmetic,
and adjacent f64 neighbors (1,920 cases). The cursor confirms that all input f64
values survive CSV reading unchanged. Numeric GARS outputs match grid-input
outputs at only 135 of 240 centers. At 239 of 240 cells the adjacent f64 changes
do not change the numeric output.

For these numeric CSV controls, 15-significant-digit decimal conversion followed
by a degree/radian roundtrip and direct five-minute tick floors predicts every
output. This is an inferred model; other source numeric types/text formats still
need probes. `diagnose_gars_numeric.py` checks the model and records 1,169 actual
boundary differences in the unchanged legacy angular-input implementation.
Those differences remain in KNOWN_GAPS; no change to that path is claimed here.

For GARS inputs, decode integer cell/quadrant/key positions in the positive grid
frame, divide by 12, then subtract 180/90 last. This arithmetic differs by ULPs
from subtracting the origin before adding local offsets. On encoding a decoded
grid point, apply the degree/radian roundtrip, add the origin, multiply by 12,
and floor directly to five-minute ticks. Longitude wraps at 180, and latitude90
is clamped to the top cell. Integer ticks then determine band/letters/quadrant/key.
No epsilon snapping or staged fractional-cell calculation is used.

This model fits all 240 saved GARS controls, including ArcGIS non-roundtrips:
116LR17 -> 115LR29 and 161AA34 -> 161AA37. The existing GEOREF degree decoder
combined with the same encoder fits its saved ordinary controls too. The retained
operation order is compatibility behavior, not an assertion about Esri internals.

## Independent validation

`make_gars_boundary_inputs.py` uses a fresh seed (91762) to produce 480 random
GARS and 480 random GEOREF cases. Systematic GARS subcells, 5/6-character forms,
edge coordinates, GEOREF precisions 2..9, adjacent minute units and invalid rows
bring totals to 652 GARS and 492 GEOREF rows. `probe_gars_holdout.py` creates typed
DBF fixtures with explicit ID/COORD columns, then probes GARS outputs. This avoids
the earlier unrelated CSV auto-coordinate schema inference. Raw oracle evidence:
`gars_holdout_results.json`; numeric diagnostic data: `gars_numeric_results.json`.

The inferred model fits all 1,144 independent outputs exactly. The native CLI
matches those plus the old 240 controls (1,384 exact strings), checked by
`compare_gars_controls.py`. Four active reference cases cover 1,312 rows: 81/87
ordinary inputs and the 652/492 typed holdouts. The ordinary ArcGIS outputs were
reused from `unsupported_gars_outputs/` after source attributes/counts were
verified unchanged; the research copies remain as historical evidence. Only the
two new typed-table reference cases needed fresh generation.

The new native policy applies to grid inputs only. GARS decoding uses the positive
frame for every output; all previous grid outputs/carry controls are regression
checked. The source ObjectID policy remains intact. The safety test retains eight
unsupported projected-grid combinations, preserving every existing output byte.

Reproduce (ArcGIS scripts via the bridge, large responses written to disk):

```
python3 scripts/research/coordinate_notation/grid/make_gars_boundary_inputs.py
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/probe_gars_numeric.py
cp /tmp/unigis-gars-numeric-results.json scripts/research/coordinate_notation/grid/gars_numeric_results.json
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/coordinate_notation/grid/probe_gars_holdout.py
cp /tmp/unigis-gars-holdout-results.json scripts/research/coordinate_notation/grid/gars_holdout_results.json
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ccn_grid_input_boundary_*'
python3 scripts/research/coordinate_notation/grid/compare_gars_controls.py
python3 scripts/research/coordinate_notation/compare.py --prefix ccn_grid_input_ --count 22
python3 scripts/research/coordinate_notation/grid/diagnose_gars_numeric.py
```

`gars_model.py`, `gars_encoder_candidates.py` and `gars_decoder_candidates.py`
retain the tested hypotheses and rejected arithmetic variants. Full gate results
are recorded in HANDOFF.md; no comparison tolerances were relaxed.

The full format gate also exposed two instances of the existing SBN ancestor-
retention gap. Coordinate outputs are unaffected. Exact entries/tree checks and
observed node moves are documented in `docs/formats/shapefile-arcgis.md`.
