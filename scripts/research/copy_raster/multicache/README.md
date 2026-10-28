# Multiband cached extrema and floating unary operations

ArcGIS Pro3.7 is the GP oracle.20 saved references:8`cprmulti_*` copies and
12`ummulti_*` Abs/Negate/Square cases (F64 incomplete/complete caches plus F32).
GDAL only creates/reads TIFF file storage and metadata.

```
python scripts/research/copy_raster/multicache/recreate_snapshots.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cprmulti_*' 'ummulti_*'
python scripts/research/copy_raster/multicache/compare_native.py
```

Pass a private output folder to recreate_snapshots.py to avoid touching committed
inputs. Original private probes use /tmp/unigis-copy-multicache-controls; the
original make_private_inputs.py builds F32 inputs and probe.py supplies the eight
copy runs. probe_types.py uses F64 variants prepared by GDAL Translate as in the
snapshot recipe. Independent *_native copies were frozen before ArcGIS ran.
The saved generator runs left all source TIFF/PAM bytes unchanged.

Input pixels are a2x2 two-band F32 fixture: band1=[1,NoData,3,4], band2=[1,2,3,4].
Deliberately stale caches distinguish preservation from recomputation. Band1 full
cache min1/max4/mean2.5/std1.2, band2 min10/max40/mean25/std12. Cases isolate first
or second cached band, extrema split across bands, moments missing in one band,
custom metadata domain and valid single-quoted XML. F64 snapshots copy the same
pixels/metadata, including the ordinary GDAL embedded-tag copy made by Translate.

Copy Raster reuses caches without PAM only when EVERY band has both extrema.
Any incomplete band causes fresh statistics for ALL bands. Partial moments in an
otherwise complete cache still use the previously established0/-1 defaults.
Custom-domain extrema do not establish a default-domain cache. Single quotes work.
The old reader's global string search mixed keys across bands, admitted domains
and missed single quotes; six of the original eight copies differed.

Abs/Negate/Square keep F64 if ANY band lacks cached extrema, narrow complete F64
caches toF32, and keep F32 inputs F32. The shared parser now makes Raster.statistics
Some(first-band-range) only when every band has cached extrema; no new Raster
fields or app API change is needed. Embedded/PAM key precedence is shared with
cache copying. Optional sidecar read behavior in the raster reader stays tolerant.

After reader correction all eight copies passed, but the nine initial unary cases
still dropped band2. The floating paths of these three operations now process
bands separately using the dataset's cache status, preserving per-band masks and
avoiding a whole-dataset clone for each band. Three F32 cases were added.
All20 per-band grid/type/NoData/pixel/core-stat/PAM checks pass (native_after.txt).
A native format regression additionally checks planning flags for all11 snapshots.

Other unary operators and integer multiband arithmetic, malformed/nonfinite caches,
nondefault environments, and unusual XML remain unverified. Cache values are not
recomputed when ArcGIS preserves them, even when deliberately stale. Shared parser
unit/integration coverage also checks embedded/PAM precedence and band bounds.
