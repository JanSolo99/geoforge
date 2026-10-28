# Copy Raster NoData defaults and embedded statistics

48 saved `cpru32_*`, `cprtag_*` and `cprnd_*` references cover seven UInt32
markers with/without source caches, destination types, overrides, precision
boundaries, all eight tagged source types and out-of-range integer markers.
38 earlier black-box controls and read-only file inspections remain in the JSON
artifacts. GDAL creates/reads files only; ArcGIS Pro 3.7 supplies GP behavior.

Recreate persisted source pixels while retaining original ArcGIS PAM caches:

```
python scripts/research/copy_raster/u32/recreate_snapshots.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cpru32_*' 'cprtag_*' 'cprnd_*'
python scripts/research/copy_raster/u32/compare_native.py
```

The other make/probe scripts recreate the original isolated /tmp controls.
Never rebuild input fixtures while an oracle generation or test is using them.

UInt32 default output type stays U32. High source markers first become signed32:
2147483648 -> -2147483648;4294967294 -> -2;4294967295 -> -1. For U32 output the
negative default marker has F32 precision:2147483649 -> -2147483648,
4278190079 -> -16777216,2147483775 -> -2147483520. Positive16777217 stays exact.
F64/I32 destinations retain the exact signed marker (2147483649 -> -2147483647).
Source masking still uses the original unsigned tag. Explicit overrides bypass
the signed default; the original positive2147483648 override stays positive.

An integer destination cannot store an out-of-range marker. Copy Raster retains
the converted original sentinel payload as ordinary data rather than inserting
a zero for a logical missing cell. Thus U32 output keeps the original positive
payload despite a negative tag; source F32 -1.25 copied to I16 with marker3.4e38
keeps ordinary -1. Floating/I32 destinations with representable markers instead
map the missing source cell to that marker. This is Copy Raster behavior; the
general GP zero-encoding policy and default GeoTIFF format API are unchanged.

The cache policy has a system-marker exception: cached source2147483648 with its
default U32 output reuses the source cache without PAM; adjacent/high marker
cases recalculate into PAM. Explicit original marker also reuses the cache.

## Storage finding and validation

The initial PAM-only probes incorrectly interpreted absent sidecar statistics as
absent statistics. ArcGIS embeds them in TIFF GDAL_METADATA (tag42112). Cached Copy
Raster preserves original cached fields; other no-PAM GP outputs embed newly
computed statistics. The scan found120 embedded/no-PAM references; all24 cases
without either store are all-NoData outputs. Saved case lists document the scan.
Native metadata reading merges embedded values with per-band PAM overrides.
Validation retains the PAM-placement check and now checks both stores.

Do not recompute preserved cached moments. The U32 system-marker cache keeps
COUNT5 even when the new negative marker leaves all6 raw cells valid. If a GDAL
source cache has no COUNT, ArcGIS exports COUNT0 while keeping its cached moments
(`cpr_dem_nodata_in`); calculating17700 valid source pixels failed that reference.

Expand DISTANCE uses statistics from a workspace extended n cells RIGHT and
BOTTOM, then saves the original extent. Enumerating all padding windows uniquely
matched [left0,rightn,top0,bottomn] for three cases, including COUNT891 in a750-cell
saved image. Symmetric padding failed. See probe_expand_windows.py and
expand_statistics_windows.json. compare_embedded_regressions.py covers these and
the missing-count cached-copy case; all five pass after the corrections.

Numeric-only changes reduced31/48 mismatches to six cached-statistics failures.
After embedded-cache preservation all48 pass, including type/grid/tag/cells and
stored count/min/max/mean/stddev (native_after.txt). Full workspace validation also
checks the120 embedded cases. Separate native/GDAL tests cover classic/BigTIFF,
band indexing, PAM overrides, XML escaping and empty outputs. The CLI overwrite
regression also checks that all-NoData outputs remove old PAM overrides.

Remaining scope: explicit fractional marker normalization (e.g. -9999.5 to I16),
nonfinite/unusual out-of-range source tags, partial/stale caches, other formats,
nondefault environments and wholesale ancillary metadata preservation. Native
in-place copying remains an explicit extension; ArcGIS's failing/destructive
same-path behavior is documented separately in statistics/README.md.
