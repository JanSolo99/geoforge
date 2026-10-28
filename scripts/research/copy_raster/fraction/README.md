# Copy Raster fractional markers and cells

ArcGIS Pro 3.7 black-box controls; GDAL creates/inspects TIFF files only.
44 saved `cprfrac_*` references cover explicit/default NoData, destination storage
boundaries, cached normalized-equal markers, pixel ties and preserved payloads.

```
python scripts/research/copy_raster/fraction/recreate_snapshots.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cprfrac_*'
python scripts/research/copy_raster/fraction/compare_native.py
```

The original make_inputs/probe scripts use private `/tmp` copies. The committed
cached_u8.tif.aux.xml is an authentic ArcGIS CalculateStatistics result; retain it
when recreating pixels. observations.json records the44 isolated controls.

Integer NoData markers round nearest, ties-even: -9999.25 -> -9999,
-9999.5 -> -10000; 0.5 ->0;1.5 ->2; U8 126.5 ->126,127.5/128.5 ->128.
Signed range checks use the rounded marker: I16 -32768.5 remains representable;
-32768.75 does not. U8 255.25 becomes255 and remaps old missing pixels;
255.5 becomes256 and preserves converted original sentinel payloads as data.
Default F64 source markers and explicitly equal overrides follow the same rules.
Cached U8 source127 with overrides126.75/127.25 retains its embedded cache.

Ordinary integer pixels round nearest with midpoint towards positive infinity:
[-2.5,-1.5,-0.5,0.5,1.5,2.5] ->[-2,-1,0,1,2,3]. Quarter controls distinguish
floor(value+0.5) from truncation. This rule also applies to an old source sentinel
retained as ordinary data when the destination marker is outside storage.
It is local to Copy Raster; the shared DataType.store API is unchanged.

Negative unsigned fractions expose two markers. For U8 argument-0.25/-0.5 the
TIFF tag remains the raw fraction, while cached statistics exclude rounded0.
The old source127 sentinel is preserved as ordinary127. Thus all6 raw cells are
valid by TIFF tag, yet COUNT5 and mean103.4 exclude the0 cell. Signed -0.0 behaves
as0 and remaps the old sentinel, giving COUNT4. A writer opt-in separates the
physical marker from logical masking/encoding and cached statistics.

Three additional marker-matching controls (marker_matches.json) confirm that
source values equal to the raw argument are converted normally: raw2.5 becomes3
even with argument2.5 (output marker2); raw-1.5 becomes-1 despite argument-1.5.
No extra pre-conversion mask is applied to ordinary cells.

Four additional default/explicit negative U8 controls preserve raw TIFF tags
-1.5/-1.75; arcpy Raster.noDataValue reports-1 for both. They are in
unsigned_defaults.json and probe_unsigned_defaults.py; inspect actual TIFF tags,
not the typed arcpy getter alone. These four are research controls, not goldens.

Initial39-case comparison failed31. Correct rounding reduced44 cases to just two
raw-tag failures; separating the physical tag resolves all44 (native_after.txt).
Native/GDAL tests verify the override in classic TIFF and BigTIFF without changing
samples or logical cached statistics. Full workspace validation is recorded in
HANDOFF.md. Nonfinite/unusual tags, stale/partial caches and nondefault environments
remain outside this slice.
