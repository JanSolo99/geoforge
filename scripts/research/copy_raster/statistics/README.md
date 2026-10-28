# Copy Raster statistics controls

22 ArcGIS Pro 3.7 black-box observations isolate source statistics from size,
band count and the presence of actual NoData cells. `observations.json` retains
results from the original temporary-file probes. `probe.py` reproduces those
controls on the persisted input snapshots, using explicit temporary outputs.

Six cached/uncached pairs have identical pixels, grid, CRS and NoData tags:

| Profile | Pixels | NoData |
|---|---|---|
| large | 150x120 F32 DEM | missing pixels |
| small | 3x2 F32 typed boundary values | missing pixel |
| multi | 2x2, two F32 bands | first-band missing pixel |
| later | 2x2, two F32 bands | later-band missing pixel |
| unused | 3x2 F32 values | tag present, no matching pixel |
| large_valid | 150x120 F32 DEM | no source tag |

All uncached copies write statistics. Cached tagged copies omit statistics for
every band, including the unused-marker and later-band controls. Cached untagged
copies write statistics. Thus neither size nor testing only the first band's
pixels explains the behavior.

Eight conversion/override controls on the small pair show that changing type
(F64 or I16) or changing the NoData marker (-9999) writes statistics even from
cached input. Explicit F32 output retains the default policy. Two further
controls explicitly set the original marker (-1.25), with/without the original
F32 type: statistics remain omitted. Compare effective output metadata rather
than the mere presence of optional parameters.

The six cached `.aux.xml` sidecars are original ArcGIS CalculateStatistics
outputs. `make_inputs.py` recreates pixels with GDAL file IO and restores those
sidecars; it never calls GDAL statistics or uses GDAL as a GP oracle. Do not run
`gdalinfo -stats` on these inputs: that changes the policy being tested.

```
python scripts/research/copy_raster/statistics/make_inputs.py
uv run -q --with 'mcp>=2' python tools/arcgis-bridge/run_code.py scripts/research/copy_raster/statistics/probe.py
```

Controls cover ordinary F32 source tags, valid cached statistics and default
raster environments. Partial/stale caches, NaN markers, unusual integer tags,
other formats and nondefault environments remain unverified. Tagged U32 signed
export and extreme/fractional override mapping remain separate known gaps.

Overwrite follow-up: `probe_overwrite.py` first copies the untagged DEM (count18000,
mean238.346012569), then replaces the same output with small_cached. ArcGIS clears
statistics (`overwrite_arcgis.jsonl`). Native CLI currently retains the previous
sidecar, despite the new 3x2 raster. This is a separate output-lifecycle defect;
the 22 saved fresh-output references do not exercise an existing destination.
