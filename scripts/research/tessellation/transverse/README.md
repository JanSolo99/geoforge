# Generate Tessellation: transverse hexagons

ArcGIS Pro3.7 black-box reference: six `tess_trhex_*` cases cover varied projected
extents/areas, a shifted origin, a one-meter extent and a thin strip. Counts are
20,20,36,21,1,5. OGR only reads files for comparison.

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'tess_trhex_*'
python scripts/research/tessellation/transverse/compare_native.py
```

For cell area A, side s=sqrt(A/(1.5*sqrt(3))), width=sqrt(3)*s. Horizontal spacing
is the width, vertical spacing1.5*s. Odd rows shift right half a width. The first
cell centre is the extent's lower-left corner. Column count is
ceil((extent_width+width/2)/width); row count ceil((extent_height+s)/(1.5*s)).

Rings are clockwise starting at the top vertex, followed by the upper-right,
lower-right, bottom, lower-left and upper-left vertices. Cells are listed bottom
row first, left to right; GRID_ID letters identify the column and the number
counts rows from the top. Id remains0, as for existing tessellation shapes.

All six native cases match count, fields, order and ordered ring coordinates at
ArcGIS's0.001m tolerance (native_after.txt). Standard reference validation also
checks schema and geometry. The original isolated probe and observations include
two additional hectare/km equivalence controls; those are ArcGIS research for the
separate area-unit correction, not claims about the current numeric-only parser.

H3 hexagons, nondefault environments, geographic/compound coordinate systems,
extreme extents and explicit area-unit conversion remain outside this slice.
