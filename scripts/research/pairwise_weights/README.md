# Assign Weights By Pairwise Comparison

ArcGIS Pro 3.7 black-box controls: `awp_*` in the reference generator/manifest.
Six successful cases cover default equal comparisons, three/four/nine variables,
fractional comparisons and WEIGHTS_ONLY. Four error cases cover variable count,
invalid names, pair order and out-of-range values. Artifacts were generated in
the preceding Claude run; this continuation reuses them.

Weights match row means after each matrix column is divided by its sum, rather
than a principal eigenvector. Reverse comparisons are reciprocals; the diagonal
is one. The direct matrix string begins `Variables` and enumerates upper-triangle
pairs in input order. Comparisons range from 1/9 through 9, including fractions.
The DBF stores Variables as text(50), matrix/Weights as Double(13,6), with
precision 12. Numerical oracle assertions also exercise unrounded weights.

Implemented: 2–9 variables, default equal weights, ordered pair strings, ADD_MATRIX
and WEIGHTS_ONLY. Matrix input from an external table/file and consistency-ratio
UI are outside this slice. Variable names that collide with output field names
or require DBF truncation remain unverified.

Validation: `scripts/check` runs the saved cases and unit controls. Regeneration:
`uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'awp_*'`.

Public reference: https://doc.esri.com/en/arcgis-pro/latest/tool-reference/analysis/assign-weights-by-pairwise-comparison.html
