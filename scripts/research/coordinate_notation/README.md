# Convert Coordinate Notation: inputs

ArcGIS Pro 3.7 is the behavior oracle. `angular_probe_results.json` preserves
black-box probe output recovered from the interrupted coordinator run on
2026-10-28. No implementation was present at that checkpoint.

The `ccn_input_*` cases in `scripts/make-arcgis-reference.py` reproduce angular
inputs from `testdata/geoproc/angular_*.csv`: six input formats, each written as
DD_NUMERIC and DMS_1. Run them together with:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'ccn_input_*'
```

Observed syntax includes prefix/suffix and lowercase hemispheres, hemisphere
sign overrides, reversed axis order, degree/minute/second punctuation, packed
DDM/DMS and minutes/seconds above 60. Single-field text defaults to latitude
then longitude; two-field inputs use X then Y. Both axes must have the same
component count. Leading decimal points, colons, mixed packed/split axes,
duplicate hemisphere axes and out-of-range coordinates fail conversion.
Invalid rows remain in the output with NULL geometry, zero numeric coordinates
or empty notation text, and warning 001293.

GARS/GEOREF grid inputs now support DD_NUMERIC, DD/DDM/DMS text, GARS and GEOREF outputs; see [grid/README.md](grid/README.md)
for precision, overflow and syntax findings and 22 additional reference cases.
Projected grid outputs from these inputs are explicitly refused pending boundary validation;
[grid/carry.md](grid/carry.md) records the resolved DDM/DMS carry rule.
Projected grid inputs (UTM, MGRS, USNG), output projections other than WGS84,
field-name collisions and the full ArcGIS permissive grammar still need work. Existing grid
outputs from valid longitude/latitude remain available. GDAL is used only to
inspect generated file contents, never to define coordinate conversion behavior.

Scoped validation: `python3 scripts/research/coordinate_notation/compare.py`
passes all 12 cases, comparing exact schema/text/NULL geometry and numeric values
within 1e-8 degrees. This uses the actual CLI and saved ArcGIS outputs. Native
parser regression: `cargo test -j3 -p unigis-tools angular_notation` passes.

CLI example (a single DMS field; `y_field` is omitted):

```
ug run convert_coordinate_notation input=testdata/geoproc/angular_dms_1.csv x_field=COORDX input_format=DMS_1 output_format=DD_NUMERIC output=/tmp/coordinates.shp
```

Full validation: `scripts/check` EXIT 0 on 2026-10-29 (203 Rust tests passed,
5 ignored, no failures; 8 Python dashboard tests, fmt and clippy -D warnings
passed). Includes the generic ArcGIS reference and geodatabase cross-checks.
Log: `/tmp/unigis-ccn-input-check2.log`. The first attempt stopped at a clippy
collapsible_if warning, corrected before this successful run.
