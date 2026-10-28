# Floating multiband Square Root, Ln and Exp

ArcGIS Pro 3.7 supplies geoprocessing behavior. GDAL only creates/reads TIFFs.
Twelve private controls cover F32/F64, cached/uncached, negative and zero domains,
float32 exponential overflow, and independent NoData positions in each band.
All ArcGIS outputs retain both bands and use F32. Native guard baseline refuses
all twelve requests without output; saved in baseline.json.

profiles.json and recreate_inputs.py build exact format inputs and verify stored
samples/type/NoData. Frozen ArcGIS CalculateStatistics PAM files restore cached
variants. The original cached TIFFs are byte-identical to their raw pair; cache
metadata is separate. Scanning samples is not used to emulate cached statistics.

```
/usr/bin/python3 scripts/research/unary_multiband/floating/recreate_inputs.py /tmp/new-float-math
```

probe.py uses /tmp/unigis-float-unary-more. It calls ArcGIS CalculateStatistics
only on private *_cached copies. Twelve umfloatmore_* goldens were generated using the repository inputs:

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'umfloatmore_*'
/usr/bin/python3 scripts/research/unary_multiband/floating/compare_native.py
/usr/bin/python3 scripts/research/unary_multiband/floating/compare_native.py /tmp/new-float-math
```

All12 pass with repository AND freshly recreated inputs, including every band's
exact samples, masks/type/NoData, statistics, grid, semantic CRS and PAM placement
(native_after.txt/recipe_verification.txt). Source hashes and frozen PAM bytes
remain unchanged after reference generation (source_verification.json).
The existing per-band dispatcher now supports these three floating operations.
The safety regression retains six refusals, moving its float cases to still-
unsupported Log2/Sin/RoundUp. Cached integer and remaining multiband operations
remain refused. Full gate/push validation is recorded in docs/HANDOFF.md.
