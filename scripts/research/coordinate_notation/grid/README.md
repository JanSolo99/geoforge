# GARS and GEOREF inputs

ArcGIS Pro 3.7 is the behavior oracle. This slice implements GARS/GEOREF inputs
with DD_NUMERIC, DD/DDM/DMS text, GARS and GEOREF outputs. Projected grid formats
from these inputs are refused before writing, pending boundary validation.
The carry and GARS follow-ups are documented in [carry.md](carry.md) and
[gars-boundaries.md](gars-boundaries.md).
Existing angular-input paths are preserved.

Black-box scripts run through:

```
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py <script>
```

`probe.py` and `probe_limits.py` establish syntax/precision; `probe_outputs.py`
checks grid outputs; `probe_boundaries.py` adds an independent 240-cell GARS
holdout and fine-precision GEOREF controls. `probe_carry*.py` isolate text carry.
Saved results are alongside each script (`*_results.txt` or JSON).

GARS decodes the southwest corner at cell (5 characters), quadrant (6) or keypad
(7) precision. Latitude uses A..Q for the first letter and A..Z for the second,
skipping I/O; longitude bands are 001..720. Quadrants are 1..4 and keypad 1..9.
Whitespace and case are ignored. After a valid seven-character prefix, remaining
characters are ignored, including nonnumeric text. Invalid prefixes retain their
row with NULL geometry and empty/zero notation fields, warning 001293.

GEOREF uses longitude tiles A..Z, latitude A..M and degree letters A..Q, skipping
I/O. Each axis requires 2..9 minute digits (first two are whole minutes).
Whitespace and case are ignored. Fractional precision and whole-minute overflow
are retained, e.g. PKAA6040 = 16E, 45 degrees 40 minutes N. Overflow beyond the
geographic bounds retains its geometry, while the notation conversion fails
(zero numeric or empty text fields, warning 001293). Syntax failures have NULL
geometry instead. Numeric output coordinates are rounded to 8 decimal places.
No GDAL computation defines these behaviors.

`make_inputs.py` reconstructs source CSVs from the 39 strings in each existing
ArcGIS output plus grammar and precision controls. GDAL reads file strings only.
The original fixtures contain 81 GARS and 87 GEOREF rows. Six earlier cases
write DD_NUMERIC, DD_1 and GEOREF; eight more cover DDM/DMS variants. A typed
148-row precision table has four further DDM/DMS cases. Four GARS cases add
ordinary inputs and typed 652/492-row boundary fixtures (22 grid cases total).
DD_2 uses the same per-axis decimal formatting.

```
python3 scripts/research/coordinate_notation/grid/make_inputs.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ccn_grid_input_*'
python3 scripts/research/coordinate_notation/compare.py --prefix ccn_grid_input_ --count 22
```

## Findings and rejected approaches

Direct re-encoding of decoded grid edges into GARS selected neighboring cells
in 33 of the initial 326 grid-output fields. Snapping floating arithmetic to
integer five-minute ticks reduced this to two errors, but changed ArcGIS's
observed boundary choices (`116LR17` becomes `115LR29`; `161AA34` becomes
`161AA37`). The independent 240-cell holdout exposes many more non-roundtrips.
Simple arithmetic/radian candidate formulas also fail the holdout. Therefore the
snapping helper was removed. The later origin/angle/tick model now reproduces
these grid-input cases exactly; see [gars-boundaries.md](gars-boundaries.md).
The original unsuccessful approaches remain recorded as evidence:
`unsupported_gars_outputs/`, `probe_outputs_results.json`,
`probe_boundaries_results.txt`; `arithmetic_candidates.py` (run from repo root)
compares formula candidates against these ArcGIS results. GDAL is only file IO.

Ordinary 8-decimal DMS formatting also fails fine GEOREF controls. For example
PKAA049999000100000000 has longitude 15 degrees 4.9999 minutes, yet ArcGIS writes
015 05 00.00000000E, while 4.99984 minutes remains 015 04 59.99040000E.
DDM output at 59.999 minutes carries into the next degree, while 59.99 does not.
The subsequent symmetric-relative carry model now matches 5,634 controls and
enables these formats; see [carry.md](carry.md). The original oracle outputs are in
`unsupported_dms_outputs/`, with additional `probe_carry*_results.txt` controls.
These original research files remain historical evidence; current generated
DDM/DMS cases are active success cases in the reference manifest.
`compare_outputs.py` checks the original GEOREF/GARS output controls.
`compare_gars_controls.py` checks the old and independent GARS holdouts.

UTM/MGRS/USNG inputs, non-ASCII punctuation/whitespace, alternate spatial references,
field-name collisions, and the full permissive grammar remain follow-up work.
The new explicit restriction applies to grid inputs; existing angular-input
behavior and its remaining unverified corner cases are unchanged.

## File-oracle distinction

The leading-space control uncovered GDAL trimming a stored DBF character value
that ArcGIS and native reading preserve. Both ArcGIS read-backs are saved in
`probe_leading_spaces_results.txt`. The shapefile-only comparison now accounts
for exactly that difference, while native round-trip comparisons remain exact.
Evidence and reproduction: [shapefile-leading-spaces.md](../../../../docs/formats/shapefile-leading-spaces.md).

## Prior grid-to-decimal validation

`scripts/check` EXIT 0 on 2026-10-29: 205 Rust tests passed, 5 ignored;
8 dashboard Python tests, fmt and clippy -D warnings passed. Includes the full
ArcGIS comparison, file-format cross-checks, decoder controls and 18 refusal
combinations preserving output bytes. Log: `/tmp/unigis-ccn-grid-input-check4.log`.
Scoped six-case comparison passed all 504 rows (exact schema/text/presence and
warning counts, numeric tolerance 1e-8 degrees); original 12 angular cases also
passed. `compare_outputs.py` checks 163 original GEOREF fields plus safe GARS
refusals. Python syntax, manifest preservation and whitespace checks passed.

Current DDM/DMS carry and identity validation: full local gate 207 Rust passed,
5 ignored,8 Python passed; see [carry.md](carry.md). Prior grid-to-decimal counts
above describe the earlier slice.
