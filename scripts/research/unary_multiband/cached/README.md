# Cached integer multiband unary math

ArcGIS Pro3.7 is the GP oracle; GDAL only creates/reads/slices TIFF format inputs.
Thirty-six controls (1/2/3 bands) cover six integer types, reversed I16 order, nonpositive
signed8/16 endpoint ranges, and entire-dataset UInt32 Square overflow and correlated three-band outputs. Three
additional private single-band controls isolate signed8 Abs and all-NoData cache
placement. profiles.json and frozen ArcGIS CalculateStatistics PAMs reproduce the
source files; no samples are scanned to replace stored extrema for type planning.

The historical union-range prototype used native9142ac1 single-band arithmetic after GDAL sliced
each source band and PAM overrode only its extrema with the dataset union.39 of42
band results matched ArcGIS. The three failures were two Int8 Abs type/NoData
results at positive127, and absent empty statistics for an all-NoData UInt32
Square band beside a valid band. union_prototype.json records failures, not a
passing final implementation. Additional controls confirm positive127 promotes
S8 toU8/NoData255 for nonnegative output ranges; signed ranges may retain127.
Single-band Abs controls also promote. Existing source126 cases retainS8.

The SA Raster.save prototypes exposed a GP distinction: direct gp.Square_sa
recomputes statistics from stored0 pixels if an integer result band is entirely
missing and its NoData marker is outside storage range. This includes single-band
all-empty outputs. All missing markers become stored0 before GP statistics;
counts include those pixels. GP initializes a covariance vector with sample
variance on the diagonal and zero off-diagonal entries, even for correlated
bands (three-band control). Source CalculateStatistics caches can contain actual
cross-covariances and remain unchanged. gp_observations.json/probe_gp.py and
probe_cov.py record the direct GP controls. The implementation follows GP output
metadata used by the reference generator, not SA-only intermediate metadata.

The reader's Raster.statistics planning range now unions every stored band range
when complete. Per-band raw metadata remains available and unchanged via
stats_metadata::read. Missing extrema still yield an uncached planning state.
Complete multiband caches with nonfinite/inverted ranges retain cache presence
using NaN extrema; registered integer unary tools refuse them before writing.
Single-band cache-reading semantics and Raster fields are retained. The new
Output metadata policy flag does not require app source changes. The CLI safety regression checks invalid later-band
extrema and moves its remaining integer refusals to unsupported SquareRoot/Ln/Exp.

```
/usr/bin/python3 scripts/research/unary_multiband/cached/recreate_inputs.py /tmp/new-cached-math
uv run -q --with 'mcp>=2' python scripts/make-arcgis-reference.py 'umcachemulti_*'
/usr/bin/python3 scripts/research/unary_multiband/cached/compare_native.py
```

The Output covariance_statistics policy flag requests the GP PAM initialization;
the Raster layout has no new fields. App source has no Output policy literals or
field consumers (rg audit). All36 strict original/fresh-recipe comparisons PASS, including covariance vectors
(native_after.txt/recipe_verification.txt). All24 original TIFF/PAM components
remain unchanged after reference generation (source_verification.json). Full
workspace/push outcomes are recorded in docs/HANDOFF.md.
Stale caches, missing-extrema integer variants, empty-band source caches,
nondefault environments and other integer multiband unary operators remain
unsupported/unverified. Do not infer support for them from the thirty-six controls.
