# Floating multiband Log2, Sin and Round Up

Twelve direct ArcGIS Pro3.7 GP controls plus one precision fixture cover F32/F64 with and without complete
statistics, invalid Log2 domains and independent missing-cell positions. All
outputs retain2bands and use F32 (observations.json). GDAL only creates/reads
file-format controls; it is never the geoprocessing oracle or runtime dependency.

Sources reuse testdata/raster/unary_float_multiband. Recreate via
../floating/recreate_inputs.py in a fresh directory; frozen ArcGIS caches and
profiles remain shared. probe.py uses the original private inputs under
/tmp/unigis-float-unary-more and writes into /tmp/unigis-float-unary-next.

Thirteen umfloatnext_* goldens now include a32value x2band F32Log2 fingerprint.
At89, direct f64log2/log2f differ from GP by1ULP.32candidate values identify the
GP path as F32 natural log multiplied by F32 LOG2_E (32/32matches; division28/32,
direct log2 18/32). NativeLog2 now uses that path only for F32 input; F64 behavior
is unchanged. log2_precision.json/log2_fingerprint.json and probe_precision.py
preserve the arithmetic evidence; recreate_precision.py restores the format
fixture from precision_values.json.

```
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'umfloatnext_*'
/usr/bin/python3 scripts/research/unary_multiband/more/compare_native.py
```

All13 exact original/fresh-recipe comparisons PASS (native_after.txt,
recipe_verification.txt). Recreate shared sources with ../floating/recreate_inputs.py,
then recreate_precision.py in that SAME private directory; pass it to the
comparator as its optional argument. Shared six source components remain hash-
identical; raw sources and precision input have no PAM. New fixture byte-matches
the original private GP source (source_verification.json).
The seven-case guard moves float refusals to Log10/Cos/RoundDown; integer
SquareRoot/Ln/Exp and invalid cached Abs refusals remain. Full workspace/push
outcomes in docs/HANDOFF.md.
