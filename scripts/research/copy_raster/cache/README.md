# Copy Raster partial and stale cached statistics

11 ArcGIS Pro 3.7 black-box controls, saved as `cprcache_*` references. Inputs are
identical three-by-two U8 pixel snapshots with NoData127 and controlled PAM fields.
The TIFF pixels come from existing fixtures; the caches are deliberate file-format
inputs, not statistics computed by an alternative GP engine.

```
python scripts/research/copy_raster/cache/recreate_snapshots.py
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'cprcache_*'
python scripts/research/copy_raster/cache/compare_native.py
```

Run the recipe with `/tmp/unigis-copy-partial-controls` as its argument to create
private copies for probe.py / probe_extra.py through the ArcGIS bridge. It also
writes independent *_native copies before ArcGIS can touch the inputs. All11
original controls and saved-reference generation left input TIFF/PAM bytes
unchanged. inputs.json records every synthetic cache field; observations.json
records oracle success/messages; comparison_before.json shows the initial nine
controls (GDAL only reads their files).

For tagged unchanged-type copies, both cached extrema select embedded output
without a PAM sidecar. Missing either extremum causes fresh PAM computation.
A cached minimum/maximum pair remains sufficient even if mean/stddev are missing.
If either moment is missing, ArcGIS resets BOTH moments to mean0 and stddev-1;
it does not compute fresh values. The previous implementation retained one
incomplete moment or omitted them, producing three core-statistic mismatches.

Missing count becomes0.000000, missing median becomesnan, missing skip factors
become1, and missing excluded-values becomes an empty field. Existing fields survive, including a deliberately stale complete cache
(min20,max200,mean100,std50,COUNT999 for pixels whose actual extrema are0/255).
Existing median123, skip factors2/3, covariance7 and excluded value127 also survive.
The implementation fills defaults with entry semantics and preserves supplied
fields; mean/stddev are the coupled exception when either is absent.

All11 exact grid/type/tag/cell/core-statistic comparisons pass (native_after.txt).
The first full-key test caught a missing empty EXCLUDEDVALUES default that GDAL
GetMetadata hid from the preliminary comparison; the raw XML/native reader sees
it. The default was added. A CLI regression separately compares every stored field, including defaults,
median/skip/covariance/exclusions, against all eight embedded-cache goldens.
The other three use fresh PAM statistics; covariance and median generation for
fresh outputs remains outside this change. Malformed/nonfinite caches, incomplete
caches across multiple bands and nondefault environments remain unverified.
