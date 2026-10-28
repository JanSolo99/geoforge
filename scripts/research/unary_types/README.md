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

This bounded fix preserves F64 for uncached Abs/Negate/Square inputs (previously
all narrowed to F32). F64 NoData is -f64MAX. The additional cached-input probe
caught an initial overbroad guard before publication: ArcGIS intentionally narrows
cached F64 to F32, performing the operation at source precision before output cast.
The corrected guard checks stored statistics. Twelve um64_* cases compare exact
cells, type/grid/CRS/NoData/statistics across both paths and tagged/untagged inputs.
F32 and integer planners remain separate. Cached fixtures include ArcGIS-produced
statistics sidecars; do not omit them when copying or regenerating inputs.

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

extra_observations.json contains18 further successes (cached tiny integers, cached
F64 and extreme uncached F64). Extreme F64 Square overflows become NoData as in
the native finite filter. Cached input prototypes live under /tmp/unigis_unary_extra*.

Three exact boundary probes in boundary_observations.json confirm signed Square
46340 remains data,46341 overflows to NoData; unsigned Square65535 remains data,
65536 overflows. Uncached U32 Negate wraps through signed32, including2^31 and
2^32-1. Run make_boundary_inputs.py then probe_boundaries.py to reproduce.

Uncached integer checkpoint:39 umint_* cases cover six integer types with/without
NoData and three exact boundary files. All48 baseline native comparisons now match
(native_after_integer.txt). Abs keeps unsigned width; Negate widens8-bit toI16 and
16-bit toI32, while32-bit operands use signed32 wrapping. Square doubles width for
8/16-bit values preserving signedness, keeps32-bit operands32-bit, and marks overflow
NoData. Uncached I32 uses -2147483648; cached legacy references can use -2147483647.
Cached type planning remains separate: the old paths do not yet reproduce all
statistics-isolation cases. Do not generalize these uncached rules to cached rasters.

Regenerate: make-arcgis-reference.py 'umint_*'. Boundary source files live under
 testdata/raster/unary_types; make_boundary_inputs.py/probe_boundaries.py preserve
recipes. The unsigned Square boundary emits ArcGIS warning010663 about not building
a raster attribute table; raster creation succeeds and values remain authoritative.

## Cached integer planner

81 umcache_* cases cover27 cached source snapshots under testdata/raster/unary_cached.
Every source includes its ArcGIS CalculateStatistics sidecar. cached_inputs.json and
make_cached_inputs.py recreate the pixel files while retaining those original sidecars.
An initial72-case native comparison found a signed endpoint difference; after fixing
it all72 known outputs match (cached_native_compare.txt). The81-case reference corpus
adds missing Abs variants and symmetric signed32 endpoints.

Range inference is conservative: Abs/Square start at0 even when no actual output
zero occurs; Negate uses[-input_max,-input_min]. Small ranges use ArcGIS's packed/
8/16-bit choices. Wider ranges select signed/unsigned32 with range-aware NoData;
if both signed endpoints are needed or the range cannot fit, ArcGIS can retain
S32 with NoData -f64MAX. Pixel overflow becomes logical missing, then the explicit
GP writer policy writes0 without changing the tag or statistics. U32 Negate wraps
through signed32; signed overflow is missing. The int32 upper bound is inclusive
for signed ranges; wholly nonnegative ranges reaching that bound promote toU32.

Regenerate: make-arcgis-reference.py 'umcache_*'. All comparisons are exact per cell,
with type/NoData/grid/CRS/statistics checked. Uncached rules and cached F64 narrowing
remain separately covered by umint_* and um64_*. Malformed/stale stats, multiband
unary arithmetic and nondefault environments remain outside this checkpoint.

Uncached integer multiband follow-up:21 typed two-band cases and the per-band
dispatch extension are recorded in `../unary_multiband/README.md`. Cached integer
multiband is still incomplete: the current cached range model retains only band0.
