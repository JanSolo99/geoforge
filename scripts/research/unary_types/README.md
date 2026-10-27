# Abs / Negate / Square pixel types

ArcGIS Pro 3.7 black-box results: observations.json (48 successes: eight pixel
types, with/without tagged NoData, three operations). Typed inputs come from the
existing composite_matrix fixtures; make_matrix.py verifies their stored values.
GDAL creates/inspects files only. Probes use private source copies under /tmp.

The native-before comparison finds22 mismatches (native_before.txt). Abs already
matches all tested integers. Other differences include S32/U32 Negate wrapping,
signed Square widths/overflow and cached-statistics-dependent type decisions.
The24 statistics-isolation probes all succeeded (statistics_observations.json); do not replace the integer
planner with a guess or break the existing cached DEM references.

This first bounded fix preserves F64 for Abs/Negate/Square (previously all narrowed
to F32). F64 NoData is -f64MAX, tagged source cells map to it, and valid values retain
their full precision. Six um64_* saved cases compare every cell exactly alongside
type/grid/CRS/NoData/statistics. F32 and integer behavior remains a separate slice.

Regenerate just this checkpoint's references:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'um64_*'
```

The fix covers the existing single-band path. Other unary functions' F64 behavior,
multiband arithmetic, additional environments and overflow-boundary variants still
need probes. Private probe outputs are /tmp/unigis_unary_<Tool>_<input>.tif.

Statistics isolation: run make_statistics_inputs.py, then probe_statistics.py through
the ArcGIS bridge. Cached I8 narrow Negate usesI8 while uncached usesI16; cached
I16 narrow Square usesU16 while uncached usesI32. Cached wide I16 Square staysI32
but uses NoData -2147483647 instead of -2147483648. Cached-statistics-dependent
output selection is confirmed; its general planner is still a separate task.
